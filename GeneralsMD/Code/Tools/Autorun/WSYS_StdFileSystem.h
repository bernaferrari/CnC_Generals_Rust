//
// Project:    WSYS Library
//
// Module:     IO
//
// File name:  wsys/StdFileSystem.h
//
// Created:    
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __WSYS_STDFILESYSTEM_H
#define __WSYS_STDFILESYSTEM_H



//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#ifndef __WSYS_FILE_H
#include "wsys_File.h"
#endif

#ifndef __WSYS_FILESYSTEM_H
#include "wsys_FileSystem.h"
#endif


//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------


//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// StdFileSystem
//===============================
/**
  *	FileSystem that maps directly to StdFile files.
	*/
//===============================

class StdFileSystem	: public FileSystem
{

	public:

		virtual					~StdFileSystem();
		virtual	File*		open( const Char *filename, Int access = 0 );		///< Creates a StdFile object and opens the file with it: See FileSystem::open


};

//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------



#endif // __WSYS_STDFILESYSTEM_H
