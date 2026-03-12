#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __SOUND_CHUNK_IDS_H
#define __SOUND_CHUNK_IDS_H

#include "saveloadids.h"
#include "definitionclassids.h"


//////////////////////////////////////////////////////////////////////////////////
//
//	These are the chunk IDs that serve as 'globally-unique' persist identifiers for
//	all persist objects inside the editor.  These are used when building the
//	PersistFactoryClass's for definitions.
//
//////////////////////////////////////////////////////////////////////////////////
enum
{
	CHUNKID_SOUND_DEF			= CHUNKID_WWAUDIO_BEGIN,
	CHUNKID_AUDIBLE_SOUND,
	CHUNKID_FILTERED_SOUND,
	CHUNKID_SOUND3D,
	CHUNKID_PSEUDO_SOUND3D,
	CHUNKID_STATIC_SAVELOAD,
	CHUNKID_DYNAMIC_SAVELOAD,
	CHUNKID_LOGICALSOUND,
	CHUNKID_LOGICALLISTENER
};


//////////////////////////////////////////////////////////////////////////////////
//
//	These are the globally-unique class identifiers that the definition system
// uses inside the editor.
//
//////////////////////////////////////////////////////////////////////////////////
enum
{
	CLASSID_SOUND_DEF			= CLASSID_SOUND,
};


#endif //__SOUND_CHUNK_IDS_H
