/////// LocalFileSystem.h ////////////////////////////////
// Bryan Cleveland, August 2002
//////////////////////////////////////////////////////////

#pragma once

#ifndef __LOCALFILESYSTEM_H
#define __LOCALFILESYSTEM_H

#include "Common/SubsystemInterface.h"
#include "FileSystem.h" // for typedefs, etc.

class File;

class LocalFileSystem : public SubsystemInterface
{
public:
	virtual ~LocalFileSystem() {}

	virtual void init() = 0;
	virtual void reset() = 0;
	virtual void update() = 0;

	virtual File * openFile(const Char *filename, Int access = 0) = 0;
	virtual Bool doesFileExist(const Char *filename) const = 0;
	virtual void getFileListInDirectory(const AsciiString& currentDirectory, const AsciiString& originalDirectory, const AsciiString& searchName, FilenameList &filenameList, Bool searchSubdirectories) const = 0; ///< search the given directory for files matching the searchName (egs. *.ini, *.rep).  Possibly search subdirectories.
	virtual Bool getFileInfo(const AsciiString& filename, FileInfo *fileInfo) const = 0; ///< see FileSystem.h
	virtual Bool createDirectory(AsciiString directory) = 0; ///< see FileSystem.h

protected:
};

extern LocalFileSystem *TheLocalFileSystem;

#endif // __LOCALFILESYSTEM_H