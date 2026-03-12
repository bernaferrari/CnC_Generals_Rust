#ifndef BCHANNEL_H
#define BCHANNEL_H

#ifndef ALWAYS_H
#include "always.h"
#endif

#ifndef BITTYPE_H
#include "bittype.h"
#endif

#ifndef CHUNKIO_H
#include "chunkio.h"
#endif

#ifndef VECTOR_H
#include "vector.h"
#endif

#ifndef W3D_FILE_H
#include "w3d_file.h"
#endif

class LogDataDialogClass;

class BitChannelClass
{
public:

	BitChannelClass(uint32 id,int maxframes,uint32 chntype,bool def_val);
	~BitChannelClass(void);

	void		Set_Bit(int framenumber,bool bit);
	void		Set_Bits(BooleanVectorClass & bits);
	bool		Get_Bit(int frameidx);
	bool		Is_Empty(void) { return IsEmpty; }
	bool		Save(ChunkSaveClass & csave, bool compress);

private:

	uint32					ID;
	uint32					ChannelType;
	int						MaxFrames;
	bool						IsEmpty;

	bool						DefaultVal;
	BooleanVectorClass	Data;
	int						Begin;
	int						End;

	// Test a bit against the "default" bit
	bool is_default(bool bit);

	// This function finds the start and end of the "non-default" data
	void compute_range(void);
  
  // compress functions
	void remove_packet(W3dTimeCodedBitChannelStruct * c, uint32 packet_idx);
	uint32 find_useless_packet(W3dTimeCodedBitChannelStruct * c);
	void compress(W3dTimeCodedBitChannelStruct * c);
  
  
};


#endif