//
// Project:    WSYS Library
//
// Module:     IO
//
// File name:  wsys/StdFile.h
//
// Created:    4/23/01
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __WSYS_STDFILE_H
#define __WSYS_STDFILE_H



//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "wsys_File.h"

//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// StdFile
//===============================
/**
  *	File abstraction for standard C file operators: open, close, lseek, read, write
	*/
//===============================

class StdFile : public File
{
	protected:

		int m_handle;											///< Std C file handle
		
	public:
		
		StdFile();										
		virtual				~StdFile();


		virtual Bool	open( const Char *filename, Int access = 0 );				///< Open a fioe for access
		virtual void	close( void );																			///< Close the file
		virtual Int		read( void *buffer, Int bytes );										///< Read the specified number of bytes in to buffer: See File::read
		virtual Int		write( void *buffer, Int bytes );										///< Write the specified number of bytes from the buffer: See File::write
		virtual Int		seek( Int new_pos, seekMode mode = CURRENT );				///< Set file position: See File::seek

};




//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------


#endif // __WSYS_STDFILE_H
