/* Copyright (C) Electronic Arts Canada Inc. 1995-2002.  All rights reserved. */

#ifndef __BTRCODEX
#define __BTRCODEX 1

#ifdef __cplusplus
extern "C" {
#endif

#ifndef __CODEX_H
#error "Include codex.h before btreecodex.h"
#endif

/****************************************************************/
/*  BTR Codex                                                   */
/****************************************************************/

/* Information Functions */

CODEXABOUT *GCALL BTREE_about(void);
bool        GCALL BTREE_is(const void *compresseddata);

/* Decode Functions */

int        GCALL BTREE_size(const void *compresseddata);
#ifdef __cplusplus
int        GCALL BTREE_decode(void *dest, const void *compresseddata, int *compressedsize=0);
#else
int        GCALL BTREE_decode(void *dest, const void *compresseddata, int *compressedsize);
#endif

/* Encode Functions */

#ifdef __cplusplus
int        GCALL BTREE_encode(void *compresseddata, const void *source, int sourcesize, int *opts=0);
#else
int        GCALL BTREE_encode(void *compresseddata, const void *source, int sourcesize, int *opts);
#endif

#ifdef __cplusplus
}
#endif
#endif

