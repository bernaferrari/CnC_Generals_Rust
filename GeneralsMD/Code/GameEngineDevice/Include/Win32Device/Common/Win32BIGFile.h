/////// Win32BIGFile.h ////////////////////////////////////
// Bryan Cleveland, August 2002
///////////////////////////////////////////////////////////

#pragma once

#ifndef __WIN32BIGFILE_H
#define __WIN32BIGFILE_H

#include "Common/ArchiveFile.h"
#include "Common/AsciiString.h"
#include "Common/List.h"

class Win32BIGFile : public ArchiveFile
{
	public:
		Win32BIGFile();
		virtual ~Win32BIGFile();

		virtual Bool					getFileInfo(const AsciiString& filename, FileInfo *fileInfo) const;	///< fill in the fileInfo struct with info about the requested file.
		virtual File*					openFile( const Char *filename, Int access = 0 );///< Open the specified file within the BIG file
		virtual void					closeAllFiles( void );									///< Close all file opened in this BIG file
		virtual AsciiString		getName( void );												///< Returns the name of the BIG file
		virtual AsciiString		getPath( void );												///< Returns full path and name of BIG file
		virtual void					setSearchPriority( Int new_priority );	///< Set this BIG file's search priority
		virtual void					close( void );													///< Close this BIG file

	protected:

		AsciiString		m_name;		///< BIG file name
		AsciiString		m_path;		///< BIG file path
};

#endif // __WIN32BIGFILE_H