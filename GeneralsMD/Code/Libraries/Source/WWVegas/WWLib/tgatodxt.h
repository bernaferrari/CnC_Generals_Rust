#ifndef TGATODXT_H
#define TGATODXT_H

#pragma message ("(gth) disabling TGAtoDXTClass temporarily so I can test the WW libs merge...")
#if 0 

// Includes.
#include "always.h"
#include <windows.h>
#include <winbase.h>


// Class declerations.
class TGAToDXTClass
{
	public:
		 TGAToDXTClass();
		~TGAToDXTClass();

		enum ErrorCode {
			OK=0,
			INVALID_BIT_DEPTH,
			INVALID_SIZE,
			INVALID_ASPECT_RATIO,
			TGA_LOAD_ERROR,
			COMPRESSION_ERROR
		};

		ErrorCode Convert (const char *inputfilename, const char *outputfilename, FILETIME *writetimeptr, bool &redundantalpha, bool dontcheckalpha=false);

	protected:
		void Write (const char *outputfilename);

		FILETIME		  *WriteTimePtr;	// Time stamp of write time of DXT file.	
		unsigned char *Buffer;			// Staging buffer.
		unsigned			BufferSize;		// Size of buffer in bytes.
		unsigned			BufferCount;	// No. of bytes written to buffer.

	friend void ReadDTXnFile (DWORD count, void *buffer);
	friend void WriteDTXnFile (DWORD datacount, void *data);
};


// Externals.
extern TGAToDXTClass _TGAToDXTConverter;

#endif //0

#endif