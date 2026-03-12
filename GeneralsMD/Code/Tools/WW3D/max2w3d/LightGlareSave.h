#if defined(_MSC_VER)
#pragma once
#endif

#ifndef LIGHTGLARESAVE_H
#define LIGHTGLARESAVE_H

#include <Max.h>
#include "w3d_file.h"
#include "chunkio.h"
#include "progress.h"


/*******************************************************************************************
**
** LightGlareSaveClass - Create a Light Glare definition from a Max mesh.  In the initial
** implementation, all I need to save is the point at the pivot of the mesh.  
**
*******************************************************************************************/
class LightGlareSaveClass
{
public:

	enum {
		EX_UNKNOWN = 0,	// exception error codes
		EX_CANCEL = 1
	};

	LightGlareSaveClass(		char *						mesh_name,	
									char *						container_name,
									INode *						inode,
									Matrix3 &					exportspace,
									TimeValue					curtime,
									Progress_Meter_Class &	meter);

	int Write_To_File(ChunkSaveClass & csave);

private:
	
	W3dLightGlareStruct		GlareData;				
	
};



#endif