/**
 * @file heap.c
 * @brief bucketed bump allocator + tri-color GC
 */
#include "heap.h"

// size properly to slot each necessary type
static const u16 SLOT_SIZES[] = {
    [HEAP_TYPE_I64]    = sizeof(i64),
    [HEAP_TYPE_U64]    = sizeof(u64),
    [HEAP_TYPE_DOUBLE] = sizeof(double),
    [HEAP_TYPE_FLOAT]  = sizeof(float),
    [HEAP_TYPE_STRING] = sizeof(HeapString),
    [HEAP_TYPE_ARRAY]  = sizeof(HeapArray),
    [HEAP_TYPE_TABLE]  = sizeof(HeapTable),
    [HEAP_TYPE_OBJECT] = sizeof(HeapObject),
};

// same but for primitives so we don't have to do force boxing
static const u16 ELEM_SIZES[] = {
    [NUL]      = 0,
    [BOOL]     = sizeof(u8),
    [U64]      = sizeof(u64),
    [I64]      = sizeof(i64),
    [FLOAT]    = sizeof(float),
    [DOUBLE]   = sizeof(double),
    [OBJ]      = sizeof(HeapRef),
    [CALLABLE] = sizeof(HeapRef),
};


// bucket operations
bool bucket_init(Bucket* b, u8 type, u16 slot_size, u32 cap) {
    if (!b) return false;

    // allocate bucket slots (and provide 2 bits per slot)
    b->data = (u8*)calloc(cap, slot_size);
    b->marks = (u64*)calloc((cap + 31) / 32, sizeof(u64));

    // alloc check
    if (!b->data || !b->marks) { 
        bucket_free(b); 
        return false; 
    }

    b->used = 0;
    b->type = type;
    b->capacity = cap;
    b->slot_size = slot_size;
    b->generation = 0;
    return true;
}

// null and zero everything inside
void bucket_free(Bucket* b) {
    if (!b) return;

    free(b->data);  
    free(b->marks); 

    // fully reset to prevent stale state bugs
    memset(b, 0, sizeof(*b));
}

bool bucket_grow(Bucket* b) {
    if (!b || !b->data || !b->marks) return false;

    // double
    u32 old_cap = b->capacity;

    // should NEVER happen, but this check will exist for debugging purposes
    if (old_cap == 0) return false;
    u32 new_cap = old_cap * 2;

    // resize slots (and confirm, calloc zeroes so no need)
    size_t new_bytes = (size_t)new_cap * (size_t)b->slot_size;
    u8* new_data = (u8*)calloc(1, new_bytes);
    if (!new_data) return false;

    // make space for new marks
    u32 old_words = (old_cap + 31u) / 32u;
    u32 new_words = (new_cap + 31u) / 32u;
    u64* new_marks = (u64*)calloc((size_t)new_words, sizeof(u64));
    if (!new_marks) {
        free(new_data);
        return false;
    }

    // copy old contents into the new buffers
    memcpy(new_data, b->data, (size_t)old_cap * (size_t)b->slot_size);
    memcpy(new_marks, b->marks, (size_t)old_words * sizeof(u64));

    // commit
    free(b->data);
    free(b->marks);

    b->data = new_data;
    b->marks = new_marks;
    b->capacity = new_cap;
    return true;
}


void* bucket_alloc(Bucket* b) {
    if (!b || !b->data) return NULL;
    if (b->used >= b->capacity && !bucket_grow(b)) return NULL;

    return b->data + (b->used++ * b->slot_size);
}

void* bucket_get(Bucket* b, u32 idx) {
    if (!b || idx >= b->used) return NULL;

    // pull the slot at HeapRef location idx
    return b->data + (idx * b->slot_size);
}

void bucket_clear_marks(Bucket* b) {
    if (!b || !b->marks) return;

    // zero out every slot
    memset(b->marks, 0, ((b->capacity + 31) / 32) * sizeof(u64));
}


// heap operations
bool heap_init(Heap* h) {
    if (!h) return false;

    // DO NOT CALL THIS ON SOMETHING THATS ALREADY INITIALIZED
    memset(h, 0, sizeof(Heap));

    // calloc all buckets in a nice little row
    h->buckets = (Bucket*)calloc(HEAP_TYPE_COUNT, sizeof(Bucket));
    if (!h->buckets) return false;

    // bucket count is ALWAYS == HEAP_TYPE_COUNT
    for (u32 i = 0; i < HEAP_TYPE_COUNT; i++) {
        if (!bucket_init(&h->buckets[i], (u8)i, SLOT_SIZES[i], DEFAULT_BUCKET_CAP)) {
            heap_free(h);
            return false;
        }
        
        h->bucket_count++;
    }

    // set starter threshold to 1kb (prolly will increase...)
    h->gc_threshold = 1024;
    return true;
}

// free inner allocations for a single slot (strings, arrays, tables, objects, whatevers innere)
static void slot_destroy(Bucket* b, u32 idx) {
    if (!b || !b->data || idx >= b->used) return;
    void* slot = b->data + (idx * b->slot_size);

    switch (b->type) {
        case HEAP_TYPE_STRING: {
            HeapString* s = (HeapString*)slot;
            free(s->data);
            s->data = NULL;
            break;
        }
        case HEAP_TYPE_ARRAY: {
            HeapArray* arr = (HeapArray*)slot;
            free(arr->data);
            arr->data = NULL;
            break;
        }
        case HEAP_TYPE_TABLE: {
            HeapTable* t = (HeapTable*)slot;
            free(t->buckets);
            t->buckets = NULL;
            break;
        }
        case HEAP_TYPE_OBJECT: {
            // TODO: implement object fields (and free safely)
            break;
        }

        // primitives have no inner allocations
        default: break;
    }
}

void heap_free(Heap* h) {
    if (!h) return;

    // free inner allocations for each used slot
    for (u32 i = 0; i < h->bucket_count; i++) {
        Bucket* b = &h->buckets[i];
        for (u32 j = 0; j < b->used; j++) slot_destroy(b, j);
        bucket_free(b);
    }

    free(h->buckets);
    free(h->gray_stack);
    memset(h, 0, sizeof(Heap));
}

HeapRef heap_alloc(Heap* h, HeapType type) {
    if (!h || type >= HEAP_TYPE_COUNT) return HEAP_REF_NULL;

    // slot in appropriate type bucket, slot == used (last index in the bucket)
    Bucket* b = &h->buckets[type];
    u32 slot = b->used;

    // guard 24-bit slot limit (HeapRef can only encode 0x00FFFFFF slots)
    if (slot >= HEAP_MAX_SLOTS) return HEAP_REF_NULL;

    // if allocation somehow fails (i would assume it'd likely be due to calloc)
    // return a null ref (which will error the runtime)
    if (!bucket_alloc(b)) return HEAP_REF_NULL;

    // slot and create the ref
    h->total_allocated += b->slot_size;
    return heapref_make((u8)type, slot);
}

void* heap_deref(Heap* h, HeapRef ref) {
    if (!h || ref == HEAP_REF_NULL) return NULL;

    // validate type (will prolly remove w the bytecode verifier)
    u8 type = heapref_type(ref);
    if (type >= h->bucket_count) return NULL;

    // get the value at that slot
    return bucket_get(&h->buckets[type], heapref_slot(ref));
}

// wrapper around heapref_type with nullchecking
// TODO: determine if i need this???
HeapType heapref_type_nc(HeapRef ref) {
    return (ref == HEAP_REF_NULL) ? HEAP_TYPE_COUNT : (HeapType)heapref_type(ref);
}

// allocate a string to the heap
HeapRef heap_alloc_string(Heap* h, const char* str, u32 len) {
    if (!h || !str) return HEAP_REF_NULL;

    // allocate string data before consuming
    char* data = (char*)malloc(len + 1);
    if (!data) return HEAP_REF_NULL;

    // then copy
    memcpy(data, str, len);
    data[len] = '\0';

    // and NOW allocate the slot
    HeapRef ref = heap_alloc(h, HEAP_TYPE_STRING);
    if (ref == HEAP_REF_NULL) {
        free(data);
        return HEAP_REF_NULL;
    }

    // then safely deref (and a guard for the extra rare fucky)
    HeapString* s = (HeapString*)heap_deref(h, ref);
    if (!s) {
        free(data);
        return HEAP_REF_NULL;
    }
    s->data = data;
    s->length = len;
    s->hash = 0;

    // bump
    h->total_allocated += len + 1;
    return ref;
}

// allocate a typed (from the Type enum), fixed-capacity array to the heap
HeapRef heap_alloc_array(Heap* h, u8 elem_type, u32 cap) {
    // disallow NUL type arrays (elem_size == 0 is undefined behavior with calloc)
    if (!h || elem_type == NUL || elem_type > CALLABLE) return HEAP_REF_NULL;

    u16 elem_size = ELEM_SIZES[elem_type];

    // allocate backing buffer first
    void* data = NULL;
    if (cap > 0) {
        data = calloc(cap, elem_size);
        if (!data) return HEAP_REF_NULL;
    }

    // then allocate the slot (to avoid a leak)
    HeapRef ref = heap_alloc(h, HEAP_TYPE_ARRAY);
    if (ref == HEAP_REF_NULL) {
        free(data);
        return HEAP_REF_NULL;
    }

    // deref and fill (again with the guard)
    HeapArray* arr = (HeapArray*)heap_deref(h, ref);
    if (!arr) {
        free(data);
        return HEAP_REF_NULL;
    }
    arr->data = data;
    arr->length = 0;
    arr->capacity = cap;
    arr->elem_size = elem_size;
    arr->elem_type = (u8)elem_type;

    // bump
    h->total_allocated += cap * elem_size;
    return ref;
}


// actual gc, inlines are in the header
void heap_trace(Heap* h) {
    if (!h) return;

    while (h->gray_count > 0) {
        HeapRef ref = h->gray_stack[--h->gray_count];
        HeapType type = heapref_type_nc(ref);
        
        // trace children by type
        switch (type) {
            case HEAP_TYPE_ARRAY: {
                // TODO: mark array elements if they can be HeapRefs
                break;
            }

            case HEAP_TYPE_TABLE: {
                // TODO: mark table keys/values
                break;
            }

            case HEAP_TYPE_OBJECT: {
                // TODO: mark object fields
                break;
            }

            // primitives have no children
            default: break;
        }

        // proven reachable and fully scanned
        heap_set_color(h, ref, MARK_BLACK);
    }
}

void heap_sweep(Heap* h) {
    if (!h) return;

    // TODO: compact or free-list unmarked slots
    // for now: just bump threshold ig... everything gets freed at the end
    h->gc_threshold = h->total_allocated * 2;

    // im p sure i already ensure its 1024 but whatever
    if (h->gc_threshold < 1024) h->gc_threshold = 1024;
    h->gc_state = GC_IDLE;
}