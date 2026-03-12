#if defined(_MSC_VER)
#pragma once
#endif

#ifndef SAVELOADIDS_H
#define SAVELOADIDS_H

//////////////////////////////////////////////////////////////////////////////////
//
//	Chunk ID Ranges.  
// Here are the ranges to use for SubSystem and PersistFactory chunk id's 
// and also the chunk id's used by the save-load system itself.  If you are 
// creating a new library that is going to take advantage of this system, 
// create a new range for it here.
//
//////////////////////////////////////////////////////////////////////////////////
enum 
{
	CHUNKID_SAVELOAD_BEGIN				= 0x00000100,
		CHUNKID_SAVELOAD_DEFMGR,
		CHUNKID_TWIDDLER,
	CHUNKID_WW3D_BEGIN					= 0x00010000,
	CHUNKID_WWPHYS_BEGIN					= 0x00020000,
	CHUNKID_WWAUDIO_BEGIN				= 0x00030000,
	CHUNKID_COMBAT_BEGIN					= 0x00040000,
	CHUNKID_COMMANDO_EDITOR_BEGIN		= 0x00050000,
	CHUNKID_PHYSTEST_BEGIN				= 0x00060000,
	CHUNKID_COMMANDO_BEGIN				= 0x00070000,
	CHUNKID_WWMATH_BEGIN					= 0x00080000,
	CHUNKID_WWTRANSLATEDB_BEGIN		= 0x00090000
};


#endif
