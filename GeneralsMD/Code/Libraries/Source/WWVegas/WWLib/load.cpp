#include	"always.h"
#include	"iff.h"
#include	"lcw.h"
#include	<string.h>


/***************************************************************************
 * Uncompress_Data -- Uncompresses data from one buffer to another.        *
 *                                                                         *
 *    This routine takes data from a compressed file (sans the first two   *
 *    size bytes) and uncompresses it to a destination buffer.  The source *
 *    data MUST have the CompHeaderType at its start.                      *
 *                                                                         *
 * INPUT:   src   -- Source compressed data pointer.                       *
 *                                                                         *
 *          dst   -- Destination (paragraph aligned) pointer.              *
 *                                                                         *
 * OUTPUT:  Returns with the size of the uncompressed data.                *
 *                                                                         *
 * WARNINGS:   If LCW compression is used, the destination buffer must     *
 *             be paragraph aligned.                                       *
 *                                                                         *
 * HISTORY:                                                                *
 *   09/17/1993 JLB : Created.                                             *
 *=========================================================================*/
unsigned long __cdecl Uncompress_Data(void const *src, void *dst)
{
	unsigned int					skip;			// Number of leading data to skip.
	CompressionType	method;		// Compression method used.
	unsigned long					uncomp_size;

	if (!src || !dst) return(NULL);

	/*
	**	Interpret the data block header structure to determine
	**	compression method, size, and skip data amount.
	*/
	uncomp_size = ((CompHeaderType*)src)->Size;
	#if(AMIGA)
		uncomp_size = Reverse_Long(uncomp_size);
	#endif
	skip = ((CompHeaderType*)src)->Skip;
	#if(AMIGA)
		skip = Reverse_Word(skip);
	#endif
	method = (CompressionType) ((CompHeaderType*)src)->Method;
	src = ((char*)src) + (long)sizeof(CompHeaderType) + (long)skip;
//	src = Add_Long_To_Pointer((void *)src, (long)sizeof(CompHeaderType) + (long)skip);

	switch (method) {

		default:
		case NOCOMPRESS:
			memmove(dst, (void *) src, uncomp_size);
//			Mem_Copy((void *) src, dst, uncomp_size);
			break;

		case HORIZONTAL:
			break;

		case LCW:
			LCW_Uncomp((void *) src, (void *) dst, (unsigned long) uncomp_size);
			break;

	}

	return(uncomp_size);
}


