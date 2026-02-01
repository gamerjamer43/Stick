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
    b->data = NULL;

    free(b->marks); 
    b->marks = NULL;

    b->capacity = b->used = 0;
}

bool bucket_grow(Bucket* b) {
    if (!b || !b->data) return false;

    // double
    u32 new_cap = b->capacity * 2;
    
    // resize slots (and confirm)
    u8* new_data = (u8*)realloc(b->data, new_cap * b->slot_size);
    if (!new_data) return false;

    // zero out the new half of the bucket
    memset(new_data + b->capacity * b->slot_size, 0, b->capacity * b->slot_size);
    b->data = new_data;
    
    // make space for new marks
    u64* new_marks = (u64*)realloc(b->marks, ((new_cap + 31) / 32) * sizeof(u64));
    if (!new_marks) return false;

    // zero those too
    memset(new_marks + (b->capacity + 31) / 32, 0, 
           (((new_cap + 31) / 32) - ((b->capacity + 31) / 32)) * sizeof(u64));

    b->marks = new_marks;
    b->capacity = new_cap;
    return true;
}

void* bucket_alloc(Bucket* b) {
    if (!b) return NULL;
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
    h->bucket_count = HEAP_TYPE_COUNT;
    for (u32 i = 0; i < HEAP_TYPE_COUNT; i++) {
        // if init fails free every bucket
        if (!bucket_init(&h->buckets[i], (u8)i, SLOT_SIZES[i], DEFAULT_BUCKET_CAP)) {
            for (u32 j = 0; j < i; j++) 
                bucket_free(&h->buckets[j]);

            free(h->buckets);
            return false;
        }
    }

    // set starter threshold to 1kb (prolly will increase...)
    h->gc_threshold = 1024;
    return true;
}

void heap_free(Heap* h) {
    if (!h) return;

    for (u32 i = 0; i < h->bucket_count; i++) 
        bucket_free(&h->buckets[i]);

    free(h->buckets);
    free(h->gray_stack);
    memset(h, 0, sizeof(Heap));
}

HeapRef heap_alloc(Heap* h, HeapType type) {
    if (!h || type >= HEAP_TYPE_COUNT) return HEAP_REF_NULL;

    // slot in appropriate type bucket, slot == used (last index in the bucket)
    Bucket* b = &h->buckets[type];
    u32 slot = b->used;

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
    // allocate (and ensure it gives a ref)
    HeapRef ref = heap_alloc(h, HEAP_TYPE_STRING);
    if (ref == HEAP_REF_NULL) return ref;

    // then safely deref
    HeapString* s = (HeapString*)heap_deref(h, ref);
    s->data = (char*)malloc(len + 1);
    if (!s->data) return HEAP_REF_NULL;

    // and fill
    memcpy(s->data, str, len);
    s->data[len] = '\0';
    s->length = len;
    s->hash = 0;

    // bump and return reference
    h->total_allocated += len + 1;
    return ref;
}

// allocate an array to the heap (bytecode verifier will ensure no bounds errors)
// TODO: unfix this from u64
HeapRef heap_alloc_array(Heap* h, u32 cap) {
    HeapRef ref = heap_alloc(h, HEAP_TYPE_ARRAY);
    if (ref == HEAP_REF_NULL) return ref;

    // allocate and ensure it worked
    HeapArray* arr = (HeapArray*)heap_deref(h, ref);
    arr->data = cap ? calloc(cap, sizeof(u64)) : NULL;
    if (cap && !arr->data) return HEAP_REF_NULL;

    // zero out new slots
    arr->length = 0;
    arr->capacity = cap;
    h->total_allocated += cap * sizeof(u64);

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