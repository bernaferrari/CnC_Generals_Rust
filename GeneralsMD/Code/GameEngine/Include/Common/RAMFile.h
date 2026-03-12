//
// Project:    WSYS Library
//
// Module:     IO
//
// File name:  wsys/RAMFile.h
//
// Created:    11/08/01
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __RAMFILE_H
#define __RAMFILE_H



//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "Common/File.h"

//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// RAMFile
//===============================
/**
  *	File abstraction for standard C file operators: open, close, lseek, read, write
	*/
//===============================

class RAMFile : public File
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(RAMFile, "RAMFile")		
	protected:

		Char				*m_data;											///< File data in memory
		Int					m_pos;												///< current read position
		Int					m_size;												///< size of file in memory
		
	public:
		
		RAMFile();
		//virtual				~RAMFile();


		virtual Bool	open( const Char *filename, Int access = 0 );				///< Open a file for access
		virtual void	close( void );																			///< Close the file
		virtual Int		read( void *buffer, Int bytes );										///< Read the specified number of bytes in to buffer: See File::read
		virtual Int		write( const void *buffer, Int bytes );							///< Write the specified number of bytes from the buffer: See File::write
		virtual Int		seek( Int new_pos, seekMode mode = CURRENT );				///< Set file position: See File::seek
		virtual void	nextLine(Char *buf = NULL, Int bufSize = 0);				///< moves current position to after the next new-line

		virtual Bool	scanInt(Int &newInt);																///< return what gets read as an integer from the current memory position.
		virtual Bool	scanReal(Real &newReal);														///< return what gets read as a float from the current memory position.
		virtual Bool	scanString(AsciiString &newString);									///< return what gets read as a string from the current memory position.

		virtual Bool	open( File *file );																	///< Open file for fast RAM access
		virtual Bool	openFromArchive(File *archiveFile, const AsciiString& filename, Int offset, Int size); ///< copy file data from the given file at the given offset for the given size.
		virtual Bool	copyDataToFile(File *localFile);										///< write the contents of the RAM file to the given local file.  This could be REALLY slow.

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


#endif // __WSYS_RAMFILE_H
