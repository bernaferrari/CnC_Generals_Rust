#if defined(_MSC_VER)
#pragma once
#endif

#ifndef FRAMEGRAB_H
#define FRAMEGRAB_H

#ifndef ALWAYS_H
#include "always.h"
#endif

#if defined (_MSC_VER)
#pragma warning (push, 3)	// (gth) system headers complain at warning level 4...
#endif

#ifndef _WINDOWS_
#include "windows.h"
#endif

#ifndef _INC_WINDOWSX
#include "windowsx.h"
#endif

#ifndef _INC_VFW
#include "vfw.h"
#endif

#if defined (_MSC_VER)
#pragma warning (pop)
#endif

// FramGrab.h: interface for the FrameGrabClass class.
//
//////////////////////////////////////////////////////////////////////

class FrameGrabClass  
{
public:
	enum MODE {
		RAW,
		AVI
	};

	// depending on which mode you select, it will produce either frames or an AVI.
	FrameGrabClass(const char *filename, MODE mode, int width, int height, int bitdepth, float framerate );

	virtual ~FrameGrabClass();

	void ConvertGrab(void *BitmapPointer);
	void Grab(void *BitmapPointer);

	long * GetBuffer()			{ return Bitmap; }
	float	GetFrameRate()			{ return FrameRate; }

protected:
	const char *Filename;
	float			FrameRate;

	MODE Mode;
	long Counter; // used for incrementing filename cunter, etc.

	void GrabAVI(void *BitmapPointer);
	void GrabRawFrame(void *BitmapPointer);

	// avi settings
	PAVIFILE				AVIFile;  
	long					*Bitmap;
	PAVISTREAM			Stream;     
	AVISTREAMINFO		AVIStreamInfo;
	BITMAPINFOHEADER	BitmapInfoHeader; 

	// general purpose cleanup routine
	void CleanupAVI();

	// convert the SR image into AVI byte ordering
	void ConvertFrame(void *BitmapPointer);

};

#endif
