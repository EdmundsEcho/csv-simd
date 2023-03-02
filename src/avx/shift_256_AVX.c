// r: result
// a: src vector
// offs: number of elements to shift (must be a const)
// elem_n: number of  elements in vector (8 for "float")
#define SLL_256(r, a, offs, elem_n) \
{ \
    __m256i    *pr = (__m256i *)&r; \
    __m256i    *pa = (__m256i *)&a; \
    __m128i    r0, r1; \
    const int    size = sizeof(a) / elem_n; \
\
    if (!offs) \
        *pr = *pa; \
    else if (offs == elem_n / 2) \
        *pr = _mm256_permute2f128_si256(*pa, *pa, 0x08); \
    else if (offs >= elem_n) \
        *pr = _mm256_setzero_si256(); \
    else if (offs < elem_n / 2) \
    { \
        r0 = _mm256_castsi256_si128(*pa); \
        r1 = _mm256_extractf128_si256(*pa, 1); \
        r1 = _mm_alignr_epi8(r1, r0, (elem_n / 2 - offs) * size); \
        r0 = _mm_slli_si128(r0, offs * size); \
        *pr = _mm256_insertf128_si256(_mm256_castsi128_si256(r0), r1, 1); \
    } \
    else \
    { \
        r0 = _mm256_castsi256_si128(*pa); \
        r0 = _mm_slli_si128(r0, (offs - elem_n / 2) * size); \
        *pr = _mm256_permute2f128_si256(_mm256_castsi128_si256(r0), _mm256_castsi128_si256(r0), 0x08); \
    } \
}

#define SRL_256(r, a, offs, elem_n) \
{ \
    __m256i    *pr = (__m256i *)&r; \
    __m256i    *pa = (__m256i *)&a; \
    __m128i    r0, r1; \
    const int    size = sizeof(a) / elem_n; \
\
    if (!offs) \
        *pr = *pa; \
    else if (offs == elem_n / 2) \
        *pr = _mm256_permute2f128_si256(*pa, *pa, 0x81); \
    else if (offs >= elem_n) \
        *pr = _mm256_setzero_si256(); \
    else if (offs < elem_n / 2) \
    { \
        r0 = _mm256_castsi256_si128(*pa); \
        r1 = _mm256_extractf128_si256(*pa, 1); \
        r0 = _mm_alignr_epi8(r1, r0, offs * size); \
        r1 = _mm_srli_si128(r1, offs * size); \
        *pr = _mm256_insertf128_si256(_mm256_castsi128_si256(r0), r1, 1); \
    } \
    else \
    { \
        r1 = _mm256_extractf128_si256(*pa, 1); \
        r1 = _mm_srli_si128(r1, (offs - elem_n / 2) * size); \
        *pr = _mm256_permute2f128_si256(_mm256_castsi128_si256(r1), _mm256_castsi128_si256(r1), 0x80); \
    } \
}
