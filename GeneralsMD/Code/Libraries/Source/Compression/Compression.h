// FILE: Compression.h ///////////////////////////////////////////////////////
// Author: Matthew D. Campbell
//////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __COMPRESSION_H__
#define __COMPRESSION_H__

#include "Lib/BaseType.h"

enum CompressionType
{
	COMPRESSION_MIN = 0,
	COMPRESSION_NONE = COMPRESSION_MIN,
	COMPRESSION_REFPACK,
	COMPRESSION_MAX = COMPRESSION_REFPACK,
	COMPRESSION_NOXLZH,
	COMPRESSION_ZLIB1,
	COMPRESSION_ZLIB2,
	COMPRESSION_ZLIB3,
	COMPRESSION_ZLIB4,
	COMPRESSION_ZLIB5,
	COMPRESSION_ZLIB6,
	COMPRESSION_ZLIB7,
	COMPRESSION_ZLIB8,
	COMPRESSION_ZLIB9,
	COMPRESSION_BTREE,
	COMPRESSION_HUFF,
};

class CompressionManager
{
public:

	static Bool isDataCompressed( const void *mem, Int len );
	static CompressionType getCompressionType( const void *mem, Int len );

	static Int getMaxCompressedSize( Int uncompressedLen, CompressionType compType );
	static Int getUncompressedSize( const void *mem, Int len );

	static Int compressData( CompressionType compType, void *src, Int srcLen, void *dest, Int destLen ); // 0 on error
	static Int decompressData( void *src, Int srcLen, void *dest, Int destLen ); // 0 on error

	static const char *getCompressionNameByType( CompressionType compType );

	// For perf timers, so we can have separate ones for compression/decompression
	static const char *getDecompressionNameByType( CompressionType compType );

	static CompressionType getPreferredCompression( void );
};

#endif // __COMPRESSION_H__
