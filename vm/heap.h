/**
 * @file heap.h
 * @author Noah Mingolelli
 * @brief header for the heap implementation and gc. heap is a type bucketted system, gc marks based on tricolor and generational
 * how this works:
 * - buckets store by by TYPE, not size. that way each bucket knows what type is inside.
 * - each bucket has a bump allocator. yay o(1)
 * - mark bits are kept separate to be nice to the cache
 * - young objects get collected often, and the survivors are moved to a new bucket
 * - a HeapRef is a packed 32 bit pointer, see below
 */
#ifndef HEAP_H
#define HEAP_H

#include "typing.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

// gc colors
#define MARK_WHITE 0   // proven unreachable (or not yet seen)
#define MARK_GRAY  1   // reachable but children not scanned
#define MARK_BLACK 2   // reachable and fully scanned

// default initial capacity per bucket
#define DEFAULT_BUCKET_CAP 64

// each type on the heap gets its own bucket
typedef enum {
    HEAP_TYPE_I64 = 0,   // boxed i64s
    HEAP_TYPE_U64,       // boxed u64s
    HEAP_TYPE_DOUBLE,    // boxed doubles
    HEAP_TYPE_FLOAT,     // boxed floats
    HEAP_TYPE_STRING,    // strings (also gonna add stack strings, 8 chars can live in a register)
    HEAP_TYPE_ARRAY,     // heap arrays (also gonna do stack arrays by calloc)
    HEAP_TYPE_TABLE,     // hashtables
    HEAP_TYPE_OBJECT,    // user-defined objects

    // TODO: figure out how to make 128 bit ints stack allocated. then add heap for them later
    // HEAP_TYPE_I128    // 128 bit integers
    // HEAP_TYPE_U128    // 128 bit UNSIGNED ints

    HEAP_TYPE_COUNT      // placeholder so we know how many items in this enum
} HeapType;


// heap ref: (type << 24 | slot_index), this way one always knows the type without a deref
typedef u32 HeapRef;

// only invalid pointer sequence is all 1s
#define HEAP_REF_NULL 0xFFFFFFFF

// state
typedef enum {
    GC_IDLE = 0,   // nothing happening
    GC_MARK,       // marking roots
    GC_TRACE,      // tracing from roots
    GC_SWEEP,      // sweeping unmarked
} GCState;

// bucketing (1 bucket to a type obviously)
typedef struct {
    u8*  data;          // raw memory block (slots)
    u64* marks;         // 2 bits marking (00, 01, 10) per slot (32 per u64, then double)
    u32  capacity;      // total slots allocated
    u32  used;          // bump pointer (next free index)
    u16  slot_size;     // bytes per slot
    u8   type;          // HeapType enum
    u8   generation;    // 0 = young, 1+ = old
} Bucket;


// heap object types
typedef struct {
    char* data;         // owned char array (null terminated)
    u32   length;       // char count (excluding null)
    u32   hash;         // cached hash (INTERNING YAY)
} HeapString;

// elements are raw u64 (type info stored elsewhere or untyped)
typedef struct {
    void* data;         // element storage
    u32   length;       // current count
    u32   capacity;     // allocated slots
} HeapArray;

// hashtable stub (TODO: either write a good table impl or use someone elses)
typedef struct {
    void* buckets;      // references to each hash bucket (figure out a type and if this is a double)
    u32   count;        // current count of entries
    u32   capacity;     // total capacity (before a resize)
} HeapTable;

// user-defined, fields follow header
typedef struct {
    u16   type_id;      // type registry id
    u16   field_count;  // number of fields

    // TODO: figure out how to keep a HeapRef to ALL FIELDS (so heap objects r only 64 bits)
} HeapObject;


// heap (like the entire heap)
typedef struct {
    Bucket* buckets;        // references to each bucket
    u32     bucket_count;   // always HEAP_TYPE_COUNT

    // state handling
    GCState gc_state;
    HeapRef* gray_stack;    // objects to trace
    u32      gray_count;    // count on the gray stack
    u32      gray_cap;      // count it can hold b4 a resize

    // alloc tracking
    size_t total_allocated; // total bytes allocated
    size_t gc_threshold;    // sweep when we reach this, then 1.5-2x it
} Heap;

// bucket api
bool  bucket_init(Bucket* b, u8 type, u16 slot_size, u32 initial_capacity);
void  bucket_free(Bucket* b);
bool  bucket_grow(Bucket* b);

void* bucket_alloc(Bucket* b);
void* bucket_get(Bucket* b, u32 index);
void  bucket_clear_marks(Bucket* b); // mark all items in a bucket as white

// heap api
bool     heap_init(Heap* h);
void     heap_free(Heap* h);

HeapRef  heap_alloc(Heap* h, HeapType type);
void*    heap_deref(Heap* h, HeapRef ref);
HeapType heap_ref_type(HeapRef ref);

// easy allocation helpers
HeapRef heap_alloc_string(Heap* h, const char* str, u32 len);
HeapRef heap_alloc_array(Heap* h, u32 capacity);

// gc api
// mark a reference by color
static inline void heap_set_color(Heap* h, HeapRef ref, u8 color) {
    if (!h || ref == HEAP_REF_NULL) return;

    u8 type = heapref_type(ref);
    u32 slot = heapref_slot(ref);
    if (type >= h->bucket_count) return;

    Bucket* b = &h->buckets[type];
    if (slot >= b->used) return;

    // w = which word we want, off = its offset
    u32 w = slot / 32, off = (slot % 32) * 2;
    b->marks[w] = (b->marks[w] & ~(0x3ULL << off))  // clear the old 2 bits
     | ((u64)(color & 0x3) << off);                 // write the new 2 bits
}

// get current color of a reference
static inline u8 heap_get_color(Heap* h, HeapRef ref) {
    if (!h || ref == HEAP_REF_NULL) return MARK_WHITE;

    u8 type = heapref_type(ref);
    u32 slot = heapref_slot(ref);
    if (type >= h->bucket_count) return MARK_WHITE;

    Bucket* b = &h->buckets[type];
    if (slot >= b->used) return MARK_WHITE;
    return (b->marks[slot / 32] >> // pick the correct slot
        ((slot % 32) * 2)) & 0x3;  // move to bottom and keep bottom 2 bits
}

static inline void heap_mark_gray(Heap* h, HeapRef ref) {
    if (!h || ref == HEAP_REF_NULL || heap_get_color(h, ref) != MARK_WHITE) return;

    // mark first
    heap_set_color(h, ref, MARK_GRAY);
    if (h->gray_count >= h->gray_cap) {
        // first resize is 64
        u32 new_cap = h->gray_cap ? h->gray_cap * 2 : 64;
        
        // confirm reallocation worked (gonna make the vm panic if it didnt)
        HeapRef* ns = (HeapRef*)realloc(h->gray_stack, new_cap * sizeof(HeapRef));
        if (!ns) return;

        h->gray_stack = ns;
        h->gray_cap = new_cap;
    }

    // then add to gray stack if we can
    h->gray_stack[h->gray_count++] = ref;
}

// mark all items white (start of gc cycle)
static inline void heap_clear_marks(Heap* h) {
    if (!h) return;

    for (u32 i = 0; i < h->bucket_count; i++) 
        bucket_clear_marks(&h->buckets[i]);
}

// if above threshold return true
static inline bool heap_should_gc(Heap* h) {
    return h && h->total_allocated >= h->gc_threshold;
}

void heap_trace(Heap* h);
void heap_sweep(Heap* h);


// some helpers
// pull type from a HeapRef
static inline u8 heapref_type(HeapRef ref) {
    return (ref >> 24) & 0xFF;
}

// pull slot index from a HeapRef (type is in slot 1)
static inline u32 heapref_slot(HeapRef ref) {
    return ref & 0x00FFFFFF;
}

// pack a HeapRef (type in byte 1 then the rest in the low 3 bytes)
static inline HeapRef heapref_make(u8 type, u32 slot) {
    return ((u32)type << 24) | (slot & 0x00FFFFFF);
}

#endif