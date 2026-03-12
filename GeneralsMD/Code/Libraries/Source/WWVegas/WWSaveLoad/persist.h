#if defined(_MSC_VER)
#pragma once
#endif

#ifndef PERSIST_H
#define PERSIST_H

#include "always.h"
#include "refcount.h"
#include "postloadable.h"

class PersistFactoryClass;
class ChunkSaveClass;
class ChunkLoadClass;


//////////////////////////////////////////////////////////////////////////////////
//
//	PersistClass
//
// PersistClass defines the interface for an object to the save load system.
// Each concrete derived type of PersistClass must have an associated 
// PersistFactoryClass that basically maps a chunk_id to a constructor,
// a save function, a load function, and a on_post_load function (taken from 
// PostLoadableClass )
//
//////////////////////////////////////////////////////////////////////////////////
class PersistClass : public PostLoadableClass
{
public:

	virtual const PersistFactoryClass &	Get_Factory (void) const			= 0;
	virtual bool								Save (ChunkSaveClass &csave)		{ return true; }
	virtual bool								Load (ChunkLoadClass &cload)		{ return true; }

};



#endif
