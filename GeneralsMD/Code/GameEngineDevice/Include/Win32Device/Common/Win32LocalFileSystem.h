///// Win32LocalFileSystem.h //////////////////////////////////
// Bryan Cleveland, August 2002
///////////////////////////////////////////////////////////////

#pragma once

#ifndef __WIN32LOCALFILESYSTEM_H
#define __WIN32LOCALFILESYSTEM_H
#include "Common/LocalFileSystem.h"

class Win32LocalFileSystem : public LocalFileSystem
{
public:
	Win32LocalFileSystem();
	virtual ~Win32LocalFileSystem();

	virtual void init();
	virtual void reset();
	virtual void update();

	virtual File * openFile(const Char *filename, Int access = 0);	///< open the given file.
	virtual Bool doesFileExist(const Char *filename) const;								///< does the given file exist?

	virtual void getFileListInDirectory(const AsciiString& currentDirectory, const AsciiString& originalDirectory, const AsciiString& searchName, FilenameList &filenameList, Bool searchSubdirectories) const; ///< search the given directory for files matching the searchName (egs. *.ini, *.rep).  Possibly search subdirectories.
	virtual Bool getFileInfo(const AsciiString& filename, FileInfo *fileInfo) const;

	virtual Bool createDirectory(AsciiString directory);

protected:
};

#endif // __WIN32LOCALFILESYSTEM_H