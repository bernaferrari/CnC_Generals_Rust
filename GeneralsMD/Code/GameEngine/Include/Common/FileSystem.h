//
// Project:    GameEngine
//
// Module:     IO
//
// File name:  FileSystem.h
//
// Created:    
//
//----------------------------------------------------------------------------

#pragma once

#ifndef __FILESYSTEM_H
#define __FILESYSTEM_H

//----------------------------------------------------------------------------
//           Includes                                                      
//----------------------------------------------------------------------------

//#include "Common/File.h"
#include "Common/STLTypedefs.h"
#include "Common/SubsystemInterface.h"

//----------------------------------------------------------------------------
//           Forward References
//----------------------------------------------------------------------------
class File;

//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------

typedef std::set<AsciiString, rts::less_than_nocase<AsciiString> > FilenameList;
typedef FilenameList::iterator FilenameListIter;

//----------------------------------------------------------------------------
//           Type Defines
//----------------------------------------------------------------------------
//#define W3D_DIR_PATH "../FinalArt/W3D/"					///< .w3d files live here
//#define TGA_DIR_PATH "../FinalArt/Textures/"		///< .tga texture files live here
//#define TERRAIN_TGA_DIR_PATH "../FinalArt/Terrain/"		///< terrain .tga texture files live here
#define W3D_DIR_PATH "Art/W3D/"					///< .w3d files live here
#define TGA_DIR_PATH "Art/Textures/"		///< .tga texture files live here
#define TERRAIN_TGA_DIR_PATH "Art/Terrain/"		///< terrain .tga texture files live here
#define MAP_PREVIEW_DIR_PATH "%sMapPreviews/"	///< We need a common place we can copy the map previews to at runtime.
#define USER_W3D_DIR_PATH "%sW3D/"					///< .w3d files live here
#define USER_TGA_DIR_PATH "%sTextures/"		///< User .tga texture files live here

// the following defines are only to be used while maintaining legacy compatability
// with old files until they are completely gone and in the regular art set
#ifdef MAINTAIN_LEGACY_FILES
#define LEGACY_W3D_DIR_PATH "../LegacyArt/W3D/"				///< .w3d files live here
#define LEGACY_TGA_DIR_PATH "../LegacyArt/Textures/"	///< .tga texture files live here
#endif  // MAINTAIN_LEGACY_FILES

// LOAD_TEST_ASSETS automatically loads w3d assets from the TEST_W3D_DIR_PATH
// without having to add an INI entry.
///@todo this allows us to use the test art directory, it should be removed for FINAL release
#define LOAD_TEST_ASSETS 1
#ifdef LOAD_TEST_ASSETS
	#define ROAD_DIRECTORY		"../TestArt/TestRoad/"
	#define TEST_STRING				"***TESTING"
// the following directories will be used to look for test art
#define LOOK_FOR_TEST_ART
#define TEST_W3D_DIR_PATH "../TestArt/"					///< .w3d files live here
#define TEST_TGA_DIR_PATH "../TestArt/"		///< .tga texture files live here
#endif

struct FileInfo {
	Int sizeHigh;
	Int sizeLow;
	Int timestampHigh;
	Int timestampLow;
};

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
#include <map>

class FileSystem : public SubsystemInterface
{
  FileSystem(const FileSystem&);
  FileSystem& operator=(const FileSystem&);
  
public:
	FileSystem();
	virtual	~FileSystem();

	void init();
	void reset();
	void update();

	File* openFile( const Char *filename, Int access = 0 );		///< opens a File interface to the specified file
	Bool doesFileExist(const Char *filename) const;								///< returns TRUE if the file exists.  filename should have no directory.
	void getFileListInDirectory(const AsciiString& directory, const AsciiString& searchName, FilenameList &filenameList, Bool searchSubdirectories) const; ///< search the given directory for files matching the searchName (egs. *.ini, *.rep).  Possibly search subdirectories.
	Bool getFileInfo(const AsciiString& filename, FileInfo *fileInfo) const; ///< fills in the FileInfo struct for the file given. returns TRUE if successful.

	Bool createDirectory(AsciiString directory); ///< create a directory of the given name.

	Bool areMusicFilesOnCD();
	void loadMusicFilesFromCD();
	void unloadMusicFilesFromCD();
protected:
  mutable std::map<unsigned,bool> m_fileExist;
};

extern FileSystem*	TheFileSystem;



//----------------------------------------------------------------------------
//           Inlining                                                       
//----------------------------------------------------------------------------



#endif // __WSYS_FILESYSTEM_H
