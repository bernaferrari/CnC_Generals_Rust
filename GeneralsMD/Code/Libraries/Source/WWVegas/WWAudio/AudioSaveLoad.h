#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __AUDIO_SAVE_LOAD_H
#define __AUDIO_SAVE_LOAD_H

#include "saveloadsubsystem.h"
#include "vector.h"
#include "bittype.h"


// Singleton instances
extern class StaticAudioSaveLoadClass _StaticAudioSaveLoadSubsystem;
extern class DynamicAudioSaveLoadClass _DynamicAudioSaveLoadSubsystem;


//////////////////////////////////////////////////////////////////////////
//
//	StaticAudioSaveLoadClass
//
//////////////////////////////////////////////////////////////////////////
class StaticAudioSaveLoadClass : public SaveLoadSubSystemClass
{
public:
	
	//////////////////////////////////////////////////////////////
	//	Public constructors/destructors
	//////////////////////////////////////////////////////////////
	StaticAudioSaveLoadClass (void)				{ }
	virtual ~StaticAudioSaveLoadClass (void)	{ }

	//////////////////////////////////////////////////////////////
	//	Public methods
	//////////////////////////////////////////////////////////////
	
	// From SaveLoadSubSystemClass
	virtual uint32				Chunk_ID (void) const;
	
protected:

	//////////////////////////////////////////////////////////////
	//	Protected methods
	//////////////////////////////////////////////////////////////
	virtual bool				Contains_Data(void) const;
	virtual bool				Save (ChunkSaveClass &csave);
	virtual bool				Load (ChunkLoadClass &cload);
	virtual const char*		Name() const { return "StaticAudioSaveLoadClass"; }
};


//////////////////////////////////////////////////////////////////////////
//
//	DynamicAudioSaveLoadClass
//
//////////////////////////////////////////////////////////////////////////
class DynamicAudioSaveLoadClass : public SaveLoadSubSystemClass
{
public:
	
	//////////////////////////////////////////////////////////////
	//	Public constructors/destructors
	//////////////////////////////////////////////////////////////
	DynamicAudioSaveLoadClass (void)				{ }
	virtual ~DynamicAudioSaveLoadClass (void)	{ }

	//////////////////////////////////////////////////////////////
	//	Public methods
	//////////////////////////////////////////////////////////////
	
	// From SaveLoadSubSystemClass
	virtual uint32				Chunk_ID (void) const;
	
protected:

	//////////////////////////////////////////////////////////////
	//	Protected methods
	//////////////////////////////////////////////////////////////
	virtual bool				Contains_Data(void) const;
	virtual bool				Save (ChunkSaveClass &csave);
	virtual bool				Load (ChunkLoadClass &cload);
	virtual const char*		Name() const { return "DynamicAudioSaveLoadClass"; }

	//bool							Save_Micro_Chunks (ChunkSaveClass &csave);
	//bool							Load_Micro_Chunks (ChunkLoadClass &cload);
};


#endif //__AUDIO_SAVE_LOAD_H


