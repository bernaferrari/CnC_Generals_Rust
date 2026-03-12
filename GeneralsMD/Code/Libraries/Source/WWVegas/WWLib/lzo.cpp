#include "lzo.h"
#include "mutex.h"
#include "wwdebug.h"
#include <stdlib.h>

/*
** Work Buffer for the LZOCompressor...
*/
lzo_byte		LZOCompressor::WorkBuffer[LZO1X_MEM_COMPRESS + 1];
lzo_byte *	LZOCompressor::EOWorkBuffer = &(LZOCompressor::WorkBuffer[LZO1X_MEM_COMPRESS + 1]);

static CriticalSectionClass mutex;

#define	BUFFER_OVERRUN_TEST_VALUE	((char)0x7d)


/***********************************************************************************************
 * LZOCompressor::Compress -- compress a buffer using LZO                                      *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   7/19/99    GTH : Created.                                                                 *
 *=============================================================================================*/
int LZOCompressor::Compress
(
	const lzo_byte * in,
	lzo_uint in_len,
	lzo_byte * out,
	lzo_uint * out_len
)
{
	CriticalSectionClass::LockClass m(mutex);

#ifdef WWDEBUG
	// Debugging code to verify that the work buffer is not overrun...
	*EOWorkBuffer = BUFFER_OVERRUN_TEST_VALUE;
#endif

	int result = lzo1x_1_compress(in,in_len,out,out_len,WorkBuffer);

#ifdef WWDEBUG
	WWASSERT(*EOWorkBuffer == BUFFER_OVERRUN_TEST_VALUE);
#endif

	return result;
}


/***********************************************************************************************
 * LZOCompressor::Decompress -- decompress a buffer using LZO                                  *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   7/19/99    GTH : Created.                                                                 *
 *=============================================================================================*/
int LZOCompressor::Decompress
(
	const lzo_byte * in,
	lzo_uint in_len,
	lzo_byte * out,
	lzo_uint * out_len
)
{
	CriticalSectionClass::LockClass m(mutex);

	return lzo1x_decompress(in,in_len,out,out_len,NULL);
}
