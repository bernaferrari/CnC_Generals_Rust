#if defined(_MSC_VER)
#pragma once
#endif

#ifndef DAZZLESAVE_H
#define DAZZLESAVE_H

#include <Max.h>
#include "w3d_file.h"
#include "chunkio.h"
#include "progress.h"


/*******************************************************************************************
**
** DazzleSaveClass - Create a Dazzle definition from an INode.  Basically, we just save
** the transform and the dazzle type that the user has selected.
**
*******************************************************************************************/
class DazzleSaveClass
{
public:

	enum {
		EX_UNKNOWN = 0,	// exception error codes
		EX_CANCEL = 1
	};

	DazzleSaveClass(		char *						mesh_name,	
								char *						container_name,
								INode *						inode,
								Matrix3 &					exportspace,
								TimeValue					curtime,
								Progress_Meter_Class &	meter);

	int Write_To_File(ChunkSaveClass & csave);

private:
	
	char						W3DName[128];
	char						DazzleType[128];	
	
};







#endif //DAZZLESAVE_H

