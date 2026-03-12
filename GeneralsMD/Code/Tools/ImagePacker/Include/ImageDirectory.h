// FILE: ImageDirectory.h /////////////////////////////////////////////////////
//
// Project:    ImagePacker
//
// File name:  ImageDirectory.h
//
// Created:    Colin Day, August 2001
//
// Desc:       Image directory description for directories containing
//						 image files to pack
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __IMAGEDIRECTORY_H_
#define __IMAGEDIRECTORY_H_

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////

// FORWARD REFERENCES /////////////////////////////////////////////////////////

// TYPE DEFINES ///////////////////////////////////////////////////////////////

// ImageDirectory -------------------------------------------------------------
/** Directory to contain art files */
//-----------------------------------------------------------------------------
class ImageDirectory
{

public:

	ImageDirectory();
	~ImageDirectory();

	char *m_path;  ///< path for directory
	UnsignedInt m_imageCount;  ///< images to consider in this directory
	ImageDirectory *m_next;
	ImageDirectory *m_prev;

};

///////////////////////////////////////////////////////////////////////////////
// INLINING ///////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////
inline ImageDirectory::~ImageDirectory( void ) { delete m_path; }
inline ImageDirectory::ImageDirectory( void ) 
{ 
	m_path = NULL; 
	m_next = NULL; 
	m_prev = NULL; 
	m_imageCount = 0;
}

// EXTERNALS //////////////////////////////////////////////////////////////////

#endif // __IMAGEDIRECTORY_H_

