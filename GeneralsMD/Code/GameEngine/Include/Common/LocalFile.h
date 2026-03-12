//
// Project:    WSYS Library
//
// Module:     IO
//
// File name:  LocalFile.h
//
// Created:    4/23/01
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __LOCALFILE_H
#define __LOCALFILE_H



//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "Common/File.h"

// srj sez: this was purely an experiment in optimization.
// at the present time, it doesn't appear to be a good one.
// but I am leaving the code in for now.
// if still present in 2003, please nuke.
#define NO_USE_BUFFERED_IO
#ifdef USE_BUFFERED_IO
#include <stdio.h>
#endif

//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------


//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// LocalFile
//===============================
/**
  *	File abstraction for standard C file operators: open, close, lseek, read, write
	*/
//===============================

class LocalFile : public File
{
	MEMORY_POOL_GLUE_ABC(LocalFile)		
	private:

#ifdef USE_BUFFERED_IO
		FILE* m_file;

		enum { BUF_SIZE = 32768 };
		char m_vbuf[BUF_SIZE];	
#else
		int m_handle;											///< Local C file handle
#endif
		
	public:
		
		LocalFile();										
		//virtual				~LocalFile();


		virtual Bool	open( const Char *filename, Int access = 0 );				///< Open a file for access
		virtual void	close( void );																			///< Close the file
		virtual Int		read( void *buffer, Int bytes );										///< Read the specified number of bytes in to buffer: See File::read
		virtual Int		write( const void *buffer, Int bytes );							///< Write the specified number of bytes from the buffer: See File::write
		virtual Int		seek( Int new_pos, seekMode mode = CURRENT );				///< Set file position: See File::seek
		virtual void	nextLine(Char *buf = NULL, Int bufSize = 0);				///< moves file position to after the next new-line
		virtual Bool	scanInt(Int &newInt);																///< return what gets read in as an integer at the current file position.
		virtual Bool	scanReal(Real &newReal);														///< return what gets read in as a float at the current file position.
		virtual	Bool	scanString(AsciiString &newString);									///< return what gets read in as a string at the current file position.
		/**
			Allocate a buffer large enough to hold entire file, read 
			the entire file into the buffer, then close the file.
			the buffer is owned by the caller, who is responsible
			for freeing is (via delete[]). This is a Good Thing to
			use because it minimizes memory copies for BIG files.
		*/
		virtual char* readEntireAndClose();
		virtual File* convertToRAMFile();

};




//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------


#endif // __LOCALFILE_H
