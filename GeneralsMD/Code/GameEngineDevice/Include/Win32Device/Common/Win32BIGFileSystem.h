//////// Win32BIGFileSystem.h ///////////////////////////
// Bryan Cleveland, August 2002
/////////////////////////////////////////////////////////////

#pragma once

#ifndef __WIN32BIGFILESYSTEM_H
#define __WIN32BIGFILESYSTEM_H

#include "Common/ArchiveFileSystem.h"

class Win32BIGFileSystem : public ArchiveFileSystem
{
public:
	Win32BIGFileSystem();
	virtual ~Win32BIGFileSystem();

	virtual void init( void );
	virtual void update( void );
	virtual void reset( void );
	virtual void postProcessLoad( void );

	// ArchiveFile operations
	virtual void closeAllArchiveFiles( void );											///< Close all Archivefiles currently open

	// File operations
	virtual ArchiveFile * openArchiveFile(const Char *filename);
	virtual void closeArchiveFile(const Char *filename);
	virtual void closeAllFiles( void );															///< Close all files associated with ArchiveFiles

	virtual Bool loadBigFilesFromDirectory(AsciiString dir, AsciiString fileMask, Bool overwrite = FALSE);
protected:

};

#endif // __WIN32BIGFILESYSTEM_H