#ifndef __MESH_DEFORM_SAVE_DEFS_H
#define __MESH_DEFORM_SAVE_DEFS_H

#include <Max.h>

///////////////////////////////////////////////////////////////////////////
//
//	Constants
//
///////////////////////////////////////////////////////////////////////////
typedef enum
{
	DEFORM_CHUNK_INFO					= 0x000000001,
	DEFORM_CHUNK_SET_INFO,
	DEFORM_CHUNK_KEYFRAME_INFO,
	DEFORM_CHUNK_POSITION_DATA,
	DEFORM_CHUNK_POSITION_VERTS,
	DEFORM_CHUNK_COLOR_DATA,
	DEFORM_CHUNK_COLOR_VERTS
} DEFORM_CHUNK_IDS;

///////////////////////////////////////////////////////////////////////////
//
//	Structures
//
///////////////////////////////////////////////////////////////////////////

//
// Deform information.  Each mesh can have sets of keyframes of
//	deform info associated with it.
// 
struct DeformChunk
{
	uint32					SetCount;
	uint32					reserved[4];
};

//
// Deform set information.  Each set is made up of a series
// of keyframes.
// 
struct DeformChunkSetInfo
{	
	uint32					KeyframeCount;
	uint32					flags;
	uint32					NumVerticies;
	uint32					NumVertexColors;
	uint32					reserved[2];
};

#define DEFORM_SET_MANUAL_DEFORM	0x00000001	// set is isn't applied during sphere or point tests.

//
// Deform keyframe information.  Each keyframe is made up of
// a set of per-vert deform data.
// 
struct DeformChunkKeyframeInfo
{
	float32					DeformPercent;
	uint32					VertexCount;
	uint32					ColorCount;
	uint32					reserved[2];
};

//
// Deform data.  Contains deform information about a vertex
// in the mesh.
// 
struct DeformDataChunk
{
	uint32					VertexIndex;
	uint32					ColorIndex;
	Point3					Value;
	uint32					reserved[2];
};


#endif //__MESH_DEFORM_SAVE_DEFS_H
