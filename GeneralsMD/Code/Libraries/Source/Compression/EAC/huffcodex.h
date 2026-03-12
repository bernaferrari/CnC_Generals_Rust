/* Copyright (C) Electronic Arts Canada Inc. 1995-2002.  All rights reserved. */

#ifndef __HUFCODEX
#define __HUFCODEX 1

#ifdef __cplusplus
extern "C" {
#endif

#ifndef __CODEX_H
#error "Include codex.h before huffcodex.h"
#endif

/****************************************************************/
/*  HUF Codex                                                   */
/****************************************************************/

/* Information Functions */

CODEXABOUT *GCALL HUFF_about(void);
bool        GCALL HUFF_is(const void *compresseddata);

/* Decode Functions */

int        GCALL HUFF_size(const void *compresseddata);
#ifdef __cplusplus
int        GCALL HUFF_decode(void *dest, const void *compresseddata, int *compressedsize=0);
#else
int        GCALL HUFF_decode(void *dest, const void *compresseddata, int *compressedsize);
#endif

/* Encode Functions */

#ifdef __cplusplus
int        GCALL HUFF_encode(void *compresseddata, const void *source, int sourcesize, int *opts=0);
#else
int        GCALL HUFF_encode(void *compresseddata, const void *source, int sourcesize, int *opts);
#endif

/****************************************************************/
/*  Internal                                                    */
/****************************************************************/

#ifndef qmin
#define qmin(a,b) ((a)<(b)?(a):(b))
#endif

#ifndef qmax
#define qmax(a,b) ((a)>(b)?(a):(b))
#endif

#ifdef __cplusplus
}
#endif
#endif

