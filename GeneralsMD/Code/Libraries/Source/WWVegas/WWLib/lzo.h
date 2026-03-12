#ifndef _LZO_H
#define _LZO_H

#include	"lzoconf.h"
#include "lzo1x.h"

// Macros.

// Maximum size of any LZO compressed chunk expessed in terms of maximum size
// of UNcompressed chunk. NOTE: LZO needs an additional 16 bytes per uncompressed K.
// Note: SKB took this from lzo.h in lol3 code.
//
// (gth) The buffer that you compress to should be the size define by 
// LZO_BUFFER_SIZE(uncompressed_size).  Also, the work buffer should be of
// size: LZO1X_MEM_COMPRESS which is defined in lzo1x.h.

#define LZO_BUFFER_SIZE(s) ((s) + ((((s) / 0x400) + 1) * 16))	


int lzo1x_1_compress ( 	const lzo_byte *in,
								lzo_uint  in_len,
                         lzo_byte *out,
								lzo_uint *out_len,
                         lzo_voidp wrkmem);


int lzo1x_decompress	(  const lzo_byte *in,
								lzo_uint  in_len,
								lzo_byte *out,
								lzo_uint *out_len,
                         lzo_voidp);



//
// LZOCompressor
// Simply wraps the 'C' style lzo compression and decompression functions and 
// hides the work buffer.  So you dont have to worry about the work buffer but
// you do have to manage the compression buffer being large enough to hold the
// worst case compression: LZO_BUFFER_SIZE(uncompressed_size).
//
class LZOCompressor
{
public:

	static int Compress
	(
		const lzo_byte *	in,
		lzo_uint				in_len,
		lzo_byte *			out,
		lzo_uint *			out_len
	);

	static int Decompress
	(
		const lzo_byte *	in,
		lzo_uint				in_len,
		lzo_byte *			out,
		lzo_uint *			out_len
	);

private:

	static lzo_byte WorkBuffer[LZO1X_MEM_COMPRESS + 1];
	static lzo_byte * EOWorkBuffer;
};



#endif
