#if defined(_MSC_VER)
#pragma once
#endif

#ifndef CLASSID_H
#define CLASSID_H

#include "always.h"

/*
** enum of all the WW3D class IDs.
*/
enum 
{
	ID_INDIRECT_TEXTURE_CLASS = 0x10000,	// IndirectTextureClass					"texture.h"
	ID_VARIABLE_TEXTURE_CLASS,					// VariableTextureClass					"texture.h"
	ID_FILE_LIST_TEXTURE_CLASS,				// FileListTextureClass					"texture.h"
	ID_RESIZEABLE_TEXTURE_INSTANCE_CLASS,	// ResizeableTextureInstanceClass	"texture.h"
	ID_ANIM_TEXTURE_INSTANCE_CLASS,			// AnimTextureInstanceClass			"texture.h"
	ID_MANUAL_ANIM_TEXTURE_INSTANCE_CLASS,	// ManualAnimTextureInstanceClass	"texture.h"
	ID_TIME_ANIM_TEXTURE_INSTANCE_CLASS,	// TimeAnimTextureInstanceClass		"texture.h"
	ID_POINT_GROUP_CLASS,						// PointGroupClass						"pointgr.h"
	ID_MESH_MODEL_CLASS,							// MeshModelClass							"mesh.cpp"
	ID_CACHED_TEXTURE_FILE_CLASS,				// CachedTextureFileClass				"assetmgr.cpp"
	ID_STREAMING_TEXTURE_CLASS,				// StreamingTextureClass				"texture.h"
	ID_STREAMING_TEXTURE_INSTANCE_CLASS,	// StreamingTextureInstanceClass		"texture.h"
};


#endif
