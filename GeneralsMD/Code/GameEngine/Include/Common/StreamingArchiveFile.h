//
// Project:    RTS 3
//
// Module:     IO
//
// File name:  Common/StreamingArchiveFile.h
//
// Created:    11/08/01
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __STREAMINGARCHIVEFILE_H
#define __STREAMINGARCHIVEFILE_H



//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

#include "Common/RAMFile.h"

//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------



//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

//===============================
// StreamingArchiveFile
//===============================
/**
  *	File abstraction for standard C file operators: open, close, lseek, read, write
	*/
//===============================

class StreamingArchiveFile : public RAMFile
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(StreamingArchiveFile, "StreamingArchiveFile")		
	protected:

		File					*m_file;											///< The archive file that I came from
		Int						m_startingPos;								///< My starting position in the archive
		Int						m_size;												///< My length
		Int						m_curPos;											///< My current position.
		
	public:
		
		StreamingArchiveFile();
		//virtual				~StreamingArchiveFile();


		virtual Bool	open( const Char *filename, Int access = 0 );				///< Open a file for access
		virtual void	close( void );																			///< Close the file
		virtual Int		read( void *buffer, Int bytes );										///< Read the specified number of bytes in to buffer: See File::read
		virtual Int		write( const void *buffer, Int bytes );							///< Write the specified number of bytes from the buffer: See File::write
		virtual Int		seek( Int new_pos, seekMode mode = CURRENT );				///< Set file position: See File::seek
		
		// Ini's should not be parsed with streaming files, that's just dumb.
		virtual void	nextLine(Char *buf = NULL, Int bufSize = 0) { DEBUG_CRASH(("Should not call nextLine on a streaming file.\n")); } 
		virtual Bool	scanInt(Int &newInt) { DEBUG_CRASH(("Should not call scanInt on a streaming file.\n"));  return FALSE; } 
		virtual Bool	scanReal(Real &newReal) { DEBUG_CRASH(("Should not call scanReal on a streaming file.\n")); return FALSE; } 
		virtual Bool	scanString(AsciiString &newString) { DEBUG_CRASH(("Should not call scanString on a streaming file.\n")); return FALSE; } 

		virtual Bool	open( File *file );																	///< Open file for fast RAM access
		virtual Bool	openFromArchive(File *archiveFile, const AsciiString& filename, Int offset, Int size); ///< copy file data from the given file at the given offset for the given size.
		virtual Bool	copyDataToFile(File *localFile) { DEBUG_CRASH(("Are you sure you meant to copyDataToFile on a streaming file?")); return FALSE; }

		virtual char* readEntireAndClose() { DEBUG_CRASH(("Are you sure you meant to readEntireAndClose on a streaming file?")); return NULL; }
		virtual File* convertToRAMFile() { DEBUG_CRASH(("Are you sure you meant to readEntireAndClose on a streaming file?")); return this; }
};




//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------


#endif // __STREAMINGARCHIVEFILE_H
