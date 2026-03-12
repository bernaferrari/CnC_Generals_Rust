#if defined(_MSC_VER)
#pragma once
#endif

#ifndef COLBOXSAVE_H
#define COLBOXSAVE_H

#include <Max.h>
#include "w3d_file.h"
#include "chunkio.h"
#include "progress.h"


/*******************************************************************************************
**
** CollisionBoxSaveClass - Create an AABox or an OBBox from a Max mesh (typically the
**	artist should use a 'box' to generate this. In any case, we're just using the bounding 
** box).
**
*******************************************************************************************/
class CollisionBoxSaveClass
{
public:

	enum {
		EX_UNKNOWN = 0,	// exception error codes
		EX_CANCEL = 1
	};

	CollisionBoxSaveClass(	char *						mesh_name,	
									char *						container_name,
									INode *						inode,
									Matrix3 &					exportspace,
									TimeValue					curtime,
									Progress_Meter_Class &	meter);

	int Write_To_File(ChunkSaveClass & csave);

private:
	
	W3dBoxStruct						BoxData;				// contains same information as the W3dOBBoxStruct
	
};



#endif //COLBOXSAVE_H