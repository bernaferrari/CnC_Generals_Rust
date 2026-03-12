// compress.h
// Compress/Decompression header.
// Author: Jeff Brown, January 1999


#ifndef __compress_h
#define __compress_h

#define MAP_EXTENSION ".map"
#define LZH_EXTENSION ".nxz"
#define RUL_EXTENSION ".rul"

Bool DecompressFile		(char *infile, char *outfile);
Bool CompressFile			(char *infile, char *outfile);
Bool CompressPacket		(char *inPacket, char *outPacket);
Bool DecompressPacket	(char *inPacket, char *outPacket);
UnsignedInt CalcNewSize		(UnsignedInt rawSize);

Bool DecompressMemory		(void *inBufferVoid, Int inSize, void *outBufferVoid, Int& outSize);
Bool CompressMemory			(void *inBufferVoid, Int inSize, void *outBufferVoid, Int& outSize);

#endif __compress_h