#if defined(_MSC_VER)
#pragma once
#endif

#ifndef NULLSAVE_H
#define NULLSAVE_H


#include <Max.h>
#include "w3d_file.h"
#include "chunkio.h"
#include "progress.h"


/*******************************************************************************************
**
** NullSaveClass - Create a Null object.
**
*******************************************************************************************/
class NullSaveClass
{
public:

	enum {
		EX_UNKNOWN = 0,	// exception error codes
		EX_CANCEL = 1
	};

	NullSaveClass(				char *						mesh_name,	
									char *						container_name,
									Progress_Meter_Class &	meter);

	int Write_To_File(ChunkSaveClass & csave);

private:
	
	W3dNullObjectStruct		NullData;				
	
};




#endif //NULLSAVE_H
