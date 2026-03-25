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
    [CALLABLE] = sizeof(Func*),
};


// bucket operations
bool bucket_init(Bucket* b, u8 type, u16 slot_size, u32 cap) {
    if (!b) return false;

    // allocate bucket slots (and provide 2 bits per slot)
    b->data = (u8*)calloc(cap, slot_size);
    b->marks = (u64*)calloc((cap + 31) / 32, sizeof(u64));
    b->dirty = (u64*)calloc((cap + 63) / 64, sizeof(u64));
    b->old = (u64*)calloc((cap + 63) / 64, sizeof(u64));

    // alloc check
    if (!b->data || !b->marks || !b->dirty || !b->old) {
        bucket_free(b); 
        return false; 
    }

    b->used = 0;
    b->free_head = BUCKET_FREE_NONE;
    b->type = type;
    b->capacity = cap;
    b->slot_size = slot_size;
    b->_pad = 0;
    return true;
}

// null and zero everything inside
void bucket_free(Bucket* b) {
    if (!b) return;

    free(b->data);  
    free(b->marks); 
    free(b->dirty);
    free(b->old);

    // fully reset to prevent stale state bugs
    memset(b, 0, sizeof(*b));
}

bool bucket_grow(Bucket* b) {
    if (!b || !b->data || !b->marks || !b->dirty || !b->old) return false;

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
    u32 old_bit_words = (old_cap + 63u) / 64u;
    u32 new_bit_words = (new_cap + 63u) / 64u;
    u64* new_marks = (u64*)calloc((size_t)new_words, sizeof(u64));
    u64* new_dirty = (u64*)calloc((size_t)new_bit_words, sizeof(u64));
    u64* new_old = (u64*)calloc((size_t)new_bit_words, sizeof(u64));
    if (!new_marks || !new_dirty || !new_old) {
        free(new_data);
        free(new_marks);
        free(new_dirty);
        free(new_old);
        return false;
    }

    // copy old contents into the new buffers
    memcpy(new_data, b->data, (size_t)old_cap * (size_t)b->slot_size);
    memcpy(new_marks, b->marks, (size_t)old_words * sizeof(u64));
    memcpy(new_dirty, b->dirty, (size_t)old_bit_words * sizeof(u64));
    memcpy(new_old, b->old, (size_t)old_bit_words * sizeof(u64));

    // commit
    free(b->data);
    free(b->marks);
    free(b->dirty);
    free(b->old);

    b->data = new_data;
    b->marks = new_marks;
    b->dirty = new_dirty;
    b->old = new_old;
    b->capacity = new_cap;
    return true;
}


void* bucket_alloc(Bucket* b, u32* index) {
    if (!b || !b->data) return NULL;

    if (b->free_head != BUCKET_FREE_NONE) {
        for (u32 slot = b->free_head; slot < b->used; slot++) {
            u32 w = slot / 32, off = (slot % 32) * 2;
            if (((b->marks[w] >> off) & 0x3) != MARK_FREE) continue;

            void* ptr = b->data + ((size_t)slot * b->slot_size);
            memset(ptr, 0, b->slot_size);
            b->marks[w] &= ~(0x3ULL << off);
            bucket_set_dirty(b, slot, false);
            bucket_set_old(b, slot, false);
            if (index) *index = slot;
            return ptr;
        }

        b->free_head = BUCKET_FREE_NONE;
    }

    if (b->used >= b->capacity && !bucket_grow(b)) return NULL;
    if (index) *index = b->used;

    void* ptr = b->data + (b->used * b->slot_size);
    u32 slot = b->used++;
    u32 w = slot / 32, off = (slot % 32) * 2;
    b->marks[w] &= ~(0x3ULL << off);
    return ptr;
}

void* bucket_get(Bucket* b, u32 idx) {
    if (!b || idx >= b->used) return NULL;

    // pull the slot at HeapRef location idx
    return b->data + (idx * b->slot_size);
}

void bucket_clear_marks(Bucket* b) {
    if (!b || !b->marks) return;

    for (u32 j = 0; j < b->used; j++) {
        u32 w = j / 32, off = (j % 32) * 2;
        if (((b->marks[w] >> off) & 0x3) == MARK_FREE) continue;
        b->marks[w] &= ~(0x3ULL << off);
    }
}

static void bucket_rebuild_free_list(Bucket* b) {
    if (!b || !b->data || !b->marks) return;

    while (b->used > 0) {
        u32 slot = b->used - 1;
        u32 w = slot / 32, off = (slot % 32) * 2;
        if (((b->marks[w] >> off) & 0x3) != MARK_FREE) break;
        b->used--;
    }

    b->free_head = BUCKET_FREE_NONE;
    for (u32 j = 0; j < b->used; j++) {
        u32 w = j / 32, off = (j % 32) * 2;
        if (((b->marks[w] >> off) & 0x3) == MARK_FREE) {
            b->free_head = j;
            break;
        }
    }
}

static void bucket_free_slot(Bucket* b, u32 idx) {
    if (!b || !b->data || !b->marks || idx >= b->used) return;

    void* slot = b->data + ((size_t)idx * b->slot_size);
    memset(slot, 0, b->slot_size);
    bucket_set_dirty(b, idx, false);
    bucket_set_old(b, idx, false);

    u32 w = idx / 32, off = (idx % 32) * 2;
    b->marks[w] = (b->marks[w] & ~(0x3ULL << off)) | ((u64)MARK_FREE << off);
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

    // major gc happens every 8 minor sweeps
    h->minor_count = 0;
    h->major_interval = 8;
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
            HeapObject* obj = (HeapObject*)slot;
            free(obj->fields);
            obj->fields = NULL;
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

    Bucket* b = &h->buckets[type];
    u32 slot = 0;

    // guard 24-bit slot limit (HeapRef can only encode 0x00FFFFFF slots)
    if (b->used >= HEAP_MAX_SLOTS && b->free_head == BUCKET_FREE_NONE) return HEAP_REF_NULL;

    // if allocation somehow fails (i would assume it'd likely be due to calloc)
    // return a null ref (which will error the runtime)
    if (!bucket_alloc(b, &slot)) return HEAP_REF_NULL;

    // slot and create the ref
    h->total_allocated += b->slot_size;
    bucket_set_old(b, slot, false);
    bucket_set_dirty(b, slot, false);
    heap_set_color(h, heapref_make((u8)type, slot), MARK_WHITE);
    return heapref_make((u8)type, slot);
}

void* heap_deref(Heap* h, HeapRef ref) {
    if (!heapref_is_valid(h, ref)) return NULL;
    return bucket_get(&h->buckets[heapref_type(ref)], heapref_slot(ref));
}

// wrapper around heapref_type with nullchecking
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

// allocate a struct (just the data for proper packing/alignment)
HeapRef heap_alloc_object(Heap* h, u16 type_id, u16 field_count) {
    if (!h) return HEAP_REF_NULL;

    // allocate SAFELY (in the rare cases there really is an out of memory this is protection)
    Value* fields = NULL;
    if (field_count > 0) {
        fields = (Value*)calloc(field_count, sizeof(Value));
        if (!fields) return HEAP_REF_NULL;
    }
    HeapRef ref = heap_alloc(h, HEAP_TYPE_OBJECT);

    // no slot leaks here buddy boy
    if (ref == HEAP_REF_NULL) {
        free(fields);
        return HEAP_REF_NULL;
    }

    // deref and fill
    HeapObject* obj = (HeapObject*)heap_deref(h, ref);
    if (!obj) {
        free(fields);
        return HEAP_REF_NULL;
    }

    obj->type_id = type_id;
    obj->field_count = field_count;
    obj->fields = fields;

    // bump bump
    h->total_allocated += field_count * sizeof(Value);
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
                HeapArray* arr = (HeapArray*)heap_deref(h, ref);
                if (!arr) break;

                // only trace if elements are heap references
                if (arr->elem_type == OBJ) {
                    HeapRef* elems = (HeapRef*)arr->data;
                    for (u32 i = 0; i < arr->length; i++) {
                        heap_mark_gray(h, elems[i]);
                    }
                }

                break;
            }

            case HEAP_TYPE_TABLE: {
                // TODO: write the fuckin table lol
                break;
            }

            case HEAP_TYPE_OBJECT: {
                HeapObject* obj = (HeapObject*)heap_deref(h, ref);
                if (!obj || !obj->fields) break;

                // trace any heap refs in fields
                for (u16 i = 0; i < obj->field_count; i++) {
                    if (obj->fields[i].type == OBJ) {
                        HeapRef child;
                        memcpy(&child, obj->fields[i].val, sizeof(HeapRef));
                        heap_mark_gray(h, child);
                    }
                }
                break;
            }

            // primitives have no children
            default: break;
        }

        // proven reachable and fully scanned
        heap_set_color(h, ref, MARK_BLACK);
        heap_clear_dirty(h, ref);
    }
}

void heap_sweep(Heap* h) {
    if (!h) return;
    h->gc_state = GC_SWEEP;

    // store how much got freed (to get below the threshold)
    size_t freed = 0;

    // check each bucket for survivors
    for (u32 i = 0; i < h->bucket_count; i++) {
        Bucket* b = &h->buckets[i];
        if (!b->data || !b->marks) continue;

        for (u32 j = 0; j < b->used; j++) {
            u32 w = j / 32, off = (j % 32) * 2;
            u8 color = (b->marks[w] >> off) & 0x3;
            if (color == MARK_FREE) continue;

            // living ppl get SKIPPED
            if (color == MARK_BLACK) {
                continue;
            }

            // either white or gray. shouldn't be gray after trace but whateva
            // frees inner allocs so no leakage
            void* slot = b->data + (j * b->slot_size);
            switch (b->type) {
                case HEAP_TYPE_STRING: {
                    HeapString* s = (HeapString*)slot;
                    if (s->data) {
                        freed += s->length + 1;
                        free(s->data);
                        s->data = NULL;
                    }
                    break;
                }

                case HEAP_TYPE_ARRAY: {
                    HeapArray* arr = (HeapArray*)slot;
                    if (arr->data) {
                        freed += arr->capacity * arr->elem_size;
                        free(arr->data);
                        arr->data = NULL;
                    }
                    break;
                }

                case HEAP_TYPE_TABLE: {
                    HeapTable* t = (HeapTable*)slot;
                    if (t->buckets) {
                        // TODO: track table bucket sizes properly (tables arent even implemented. might steal a hashtable header)
                        free(t->buckets);
                        t->buckets = NULL;
                    }
                    break;
                }

                case HEAP_TYPE_OBJECT: {
                    HeapObject* obj = (HeapObject*)slot;
                    if (obj->fields) {
                        freed += obj->field_count * sizeof(Value);
                        free(obj->fields);
                        obj->fields = NULL;
                    }
                    break;
                }
                default: break;
            }

            freed += b->slot_size;
            bucket_free_slot(b, j);
        }
        bucket_rebuild_free_list(b);
    }

    // update alloc tracker
    if (freed <= h->total_allocated) h->total_allocated -= freed;
    else h->total_allocated = 0;

    // double and reset state (my balls hurt)
    h->gc_threshold = h->total_allocated * 2;
    if (h->gc_threshold < 1024) h->gc_threshold = 1024;
    h->gc_state = GC_IDLE;
}

// trace and sweep helper for MAJOR collection. caller marks roots
void heap_collect(Heap* h) {
    if (!h) return;
    h->gc_state = GC_TRACE;
    heap_trace(h);
    heap_sweep(h);

    // everything alive is now old gen
    for (u32 i = 0; i < h->bucket_count; i++) {
        Bucket* b = &h->buckets[i];
        for (u32 j = 0; j < b->used; j++) {
            if (((b->marks[j / 32] >> ((j % 32) * 2)) & 0x3) == MARK_FREE) continue;
            bucket_set_old(b, j, true);
            bucket_set_dirty(b, j, false);
        }
    }

    h->minor_count = 0;
}

// sweep only young region
void heap_sweep_young(Heap* h) {
    if (!h) return;
    h->gc_state = GC_SWEEP;

    size_t freed = 0;

    for (u32 i = 0; i < h->bucket_count; i++) {
        Bucket* b = &h->buckets[i];
        if (!b->data || !b->marks) continue;

        for (u32 j = 0; j < b->used; j++) {
            if (bucket_is_old(b, j)) continue;

            u32 w = j / 32, off = (j % 32) * 2;
            u8 color = (b->marks[w] >> off) & 0x3;
            if (color == MARK_FREE) continue;

            if (color == MARK_BLACK) {
                continue;
            }

            // free inner allocations if the obj dies young
            void* slot = b->data + (j * b->slot_size);
            switch (b->type) {
                case HEAP_TYPE_STRING: {
                    HeapString* s = (HeapString*)slot;
                    if (s->data) {
                        freed += s->length + 1;
                        free(s->data);
                        s->data = NULL;
                    }
                    break;
                }

                case HEAP_TYPE_ARRAY: {
                    HeapArray* arr = (HeapArray*)slot;
                    if (arr->data) {
                        freed += arr->capacity * arr->elem_size;
                        free(arr->data);
                        arr->data = NULL;
                    }
                    break;
                }

                case HEAP_TYPE_TABLE: {
                    HeapTable* t = (HeapTable*)slot;
                    if (t->buckets) {
                        free(t->buckets);
                        t->buckets = NULL;
                    }
                    break;
                }

                case HEAP_TYPE_OBJECT: {
                    HeapObject* obj = (HeapObject*)slot;
                    if (obj->fields) {
                        freed += obj->field_count * sizeof(Value);
                        free(obj->fields);
                        obj->fields = NULL;
                    }
                    break;
                }
                default: break;
            }

            freed += b->slot_size;
            bucket_free_slot(b, j);
        }
        bucket_rebuild_free_list(b);
    }

    if (freed <= h->total_allocated) h->total_allocated -= freed;
    else h->total_allocated = 0;

    h->gc_threshold = h->total_allocated * 2;
    if (h->gc_threshold < 1024) h->gc_threshold = 1024;
    h->gc_state = GC_IDLE;
}

// promote surviving young objects into old gen
void heap_promote_survivors(Heap* h) {
    if (!h) return;

    for (u32 i = 0; i < h->bucket_count; i++) {
        Bucket* b = &h->buckets[i];
        for (u32 j = 0; j < b->used; j++) {
            if (((b->marks[j / 32] >> ((j % 32) * 2)) & 0x3) == MARK_FREE) continue;
            bucket_set_old(b, j, true);
            bucket_set_dirty(b, j, false);
        }
    }
}

// trace, sweep babies and promote
void heap_minor_collect(Heap* h) {
    if (!h) return;
    h->gc_state = GC_TRACE;
    heap_trace(h);
    heap_sweep_young(h);
    heap_promote_survivors(h);
    h->minor_count++;
}
