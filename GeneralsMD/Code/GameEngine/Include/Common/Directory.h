// FILE: Directory.h //////////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, August 2002
// Desc:	 Directory info class
///////////////////////////////////////////////////////////////////////////////////////////////////
#if (0)
#pragma once

#ifndef __DIRECTORY_H__
#define __DIRECTORY_H__

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/AsciiString.h"
#include "Common/STLTypedefs.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////

// TYPES //////////////////////////////////////////////////////////////////////////////////////////

class FileInfo
{
public:
	FileInfo() {}
	~FileInfo() {}

	void set( const WIN32_FIND_DATA& info );

	AsciiString filename;
	time_t creationTime;
	time_t accessTime;
	time_t modTime;
	DWORD attributes;
	DWORD filesize;	// only care about 32 bits for our purposes

protected:
};

struct FileInfoComparator
{
	bool operator()(const FileInfo& a, const FileInfo& b) const
	{
		return a.filename < b.filename;
	}
};

//-------------------------------------------------------------------------------------------------

typedef std::set<FileInfo, FileInfoComparator> FileInfoSet;

//-------------------------------------------------------------------------------------------------

class Directory
{
public:
	Directory(const AsciiString& dirPath);
	~Directory() {}

	FileInfoSet* getFiles( void );
	FileInfoSet* getSubdirs( void );

protected:
	AsciiString m_dirPath;

	FileInfoSet m_files;
	FileInfoSet m_subdirs;
};

//-------------------------------------------------------------------------------------------------

#endif // __DIRECTORY_H__

#endif