//
// Project:    WSYS Library
//
// Module:     IO
//
// File name:  wsys/FileSystem.h
//
// Created:    
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __WSYS_FILESYSTEM_H
#define __WSYS_FILESYSTEM_H



//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#ifndef __WSYS_FILE_H
#include "wsys_file.h"
#endif


//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------


//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// FileSystem
//===============================
/**
  * FileSystem is an interface class for creating specific FileSystem objects.
  * 
	* A FileSystem object's implemenation decides what derivative of File object needs to be 
	* created when FileSystem::Open() gets called.
	*/
//===============================

class FileSystem
{
	protected:

	public:

		virtual					~FileSystem() {};
		virtual	File*		open( const Char *filename, Int access = 0 ) = NULL ;		///< opens a File interface to the specified file


};

extern FileSystem*	TheFileSystem;



//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------



#endif // __WSYS_FILESYSTEM_H
