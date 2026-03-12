#if defined(_MSC_VER)
#pragma once
#endif


#ifndef SAVELOADSUBSYSTEM_H
#define SAVELOADSUBSYSTEM_H


#include "always.h"
#include "bittype.h"
#include "postloadable.h"

class ChunkLoadClass;
class ChunkSaveClass;


//////////////////////////////////////////////////////////////////////////////////
//
//	SaveLoadSubSystemClass
//
// Each SaveLoadSubSystem will automatically be registered with the SaveLoadSystem 
// at construction time.  The plan is to have a single static instance of each 
// sub-system so that it automatically registers when the global constructors
// are executed.  
//
// When an application wants to create a file it does so by asking the
// SaveLoadSystem to save the particular set of SaveLoadSubSystems that contain
// that data.
//
//////////////////////////////////////////////////////////////////////////////////
class SaveLoadSubSystemClass : public PostLoadableClass
{
public:
	
	SaveLoadSubSystemClass (void);
	virtual ~SaveLoadSubSystemClass (void);

	virtual uint32				Chunk_ID (void) const = 0;

protected:

	virtual bool				Contains_Data(void) const						{ return true; }
	virtual bool				Save (ChunkSaveClass &csave) = 0;
	virtual bool				Load (ChunkLoadClass &cload) = 0;

	virtual const char*		Name() const = 0;

private:

	SaveLoadSubSystemClass *	NextSubSystem;			// managed by SaveLoadSystem

	friend class SaveLoadSystemClass;
};



#endif