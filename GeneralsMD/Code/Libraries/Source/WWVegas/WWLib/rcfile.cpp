#include "rcfile.h"
#include <stdlib.h>

const char * RESOURCE_FILE_TYPE_NAME = "File";


ResourceFileClass::ResourceFileClass(HMODULE hmodule, char const *filename) :
	ResourceName(NULL),
	hModule(NULL),
	FileBytes(NULL),
	FilePtr(NULL),
	EndOfFile(NULL)
{
	Set_Name(filename);
	HRSRC hresource = FindResource(hmodule,ResourceName,RESOURCE_FILE_TYPE_NAME);	

	if (hresource) {
		HGLOBAL hglob = LoadResource(hmodule,hresource);
		if (hglob) {
			FileBytes = (unsigned char *)LockResource(hglob);
			if (FileBytes) {
				FilePtr = FileBytes;
				EndOfFile = FileBytes + SizeofResource(hmodule,hresource);
			}
		}
	}
}

ResourceFileClass::~ResourceFileClass(void)									
{ 
	if (ResourceName) 
		free(ResourceName); 
}

char const * ResourceFileClass::Set_Name(char const *filename)
{
	if (ResourceName) {
		free(ResourceName);
		ResourceName = NULL;
	}
	if (filename) {
		ResourceName = strdup(filename);
	} 
	return ResourceName;
}

int ResourceFileClass::Read(void *buffer, int size)
{
	if (!FilePtr) return 0;

	if (FilePtr + size > EndOfFile) {
		size = EndOfFile - FilePtr;
	}
	memcpy(buffer,FilePtr,size);
	FilePtr += size;
	return size;
}

int ResourceFileClass::Seek(int pos, int dir)
{
	switch (dir) {
		case SEEK_SET:
			FilePtr = FileBytes + pos;
			break;

		case SEEK_CUR:
			FilePtr = FilePtr + pos;
			break;

		case SEEK_END:
			FilePtr = EndOfFile + pos;
			break;
	}

	if (FilePtr > EndOfFile) {
		FilePtr = EndOfFile;
	}
	if (FilePtr < FileBytes) {
		FilePtr = FileBytes;
	}
	
	return FilePtr - FileBytes;
}

int ResourceFileClass::Size(void)
{
	return EndOfFile - FileBytes;
}

void ResourceFileClass::Error(int /*error*/, int /*canretry*/, char const * /*filename*/)
{
}
