#ifndef	MIXFILE_H
#define	MIXFILE_H

#ifndef	ALWAYS_H
	#include "always.h"
#endif

#ifndef	FFACTORY_H
	#include "ffactory.h"
#endif

#ifndef	WWSTRING_H
	#include "wwstring.h"
#endif

#include "vector.h"

class FileClass;

/*
**
*/
class	MixFileFactoryClass : public FileFactoryClass {

public:
	MixFileFactoryClass( const char * mix_filename, FileFactoryClass * factory );
	virtual ~MixFileFactoryClass( void );

	//
	//	Inherited
	//
	virtual FileClass * Get_File( char const *filename );
	virtual void Return_File( FileClass *file );

	//
	//	Filename access
	//
	bool		Build_Filename_List (DynamicVectorClass<StringClass> &list);
	bool		Build_Ordered_Filename_List (DynamicVectorClass<StringClass> &list);		// ordered by offset in mixfile
	bool		Build_Internal_Filename_List (void)									{ return Build_Filename_List (FilenameList); }
	void		Get_Filename_List (DynamicVectorClass<StringClass> **list)	{ *list = &FilenameList; }
	void		Get_Filename_List (DynamicVectorClass<StringClass> &list)	{ list = FilenameList; }

	//
	//	Content control
	//
	void		Add_File (const char *full_path, const char *filename);
	void		Delete_File (const char *filename);
	void		Flush_Changes (void);

	//
	//	Information
	//
	bool		Is_Valid (void) const	{ return IsValid; }

private:

	//
	//	Utility functions
	//
	bool			Get_Temp_Filename (const char *path, StringClass &full_path);
	static int	File_Offset_Compare(const void * a, const void * b);

	struct FileInfoStruct {
		bool operator== (const FileInfoStruct &src)	{ return false; }
		bool operator!= (const FileInfoStruct &src)	{ return true; }

		unsigned long CRC;				// CRC code for embedded file.
		unsigned long Offset;			// Offset from start of data section.
		unsigned long Size;				// Size of data subfile.
	};

	struct AddInfoStruct {
		bool operator== (const AddInfoStruct &src)	{ return false; }
		bool operator!= (const AddInfoStruct &src)	{ return true; }

		StringClass FullPath;
		StringClass	Filename;
	};

	FileFactoryClass *						Factory;
	DynamicVectorClass<FileInfoStruct>	FileInfo;
	StringClass									MixFilename;
	int											BaseOffset;

	int											FileCount;
	int											NamesOffset;
	bool											IsValid;
	DynamicVectorClass<StringClass>		FilenameList;

	DynamicVectorClass<AddInfoStruct>	PendingAddFileList;
	bool											IsModified;
};

/*
**
*/
class	MixFileCreator {

public:
	MixFileCreator( const char * filename );
	~MixFileCreator( void );

	void	Add_File( const char * source_filename, const char * saved_filename = NULL );
	void	Add_File( const char * filename, FileClass *file );

private:

	static int File_Info_Compare(const void * a, const void * b);

	struct FileInfoStruct {
		bool operator== (const FileInfoStruct &src)	{ return false; }
		bool operator!= (const FileInfoStruct &src)	{ return true; }

		unsigned long	CRC;				// CRC code for embedded file.
		unsigned long	Offset;			// Offset from start of data section.
		unsigned long	Size;				// Size of data subfile.
		StringClass		Filename;
	};

	DynamicVectorClass<FileInfoStruct>	FileInfo;
	FileClass								*	MixFile;
};

/*
**
*/
void	Setup_Mix_File( void );

#endif