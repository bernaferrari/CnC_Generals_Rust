// FILE: WinMain.cpp //////////////////////////////////////////////////////////
//
// Project:    ImagePacker
//
// File name:  WinMain.cpp
//
// Created:    Colin Day, August 2001
//
// Desc:       Application entry point for the image packer tool
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
#include <windows.h>
#include <stdlib.h>

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "Lib/BaseType.h"
#include "Common/GameMemory.h"
#include "Common/Debug.h"
#include "ImagePacker.h"
#include "Resource.h"
#include "WindowProc.h"

// DEFINES ////////////////////////////////////////////////////////////////////

// PRIVATE TYPES //////////////////////////////////////////////////////////////

// PRIVATE DATA ///////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// PUBLIC DATA ////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////
HINSTANCE ApplicationHInstance = NULL;  ///< our application instance

/// just to satisfy the game libraries we link to
HWND ApplicationHWnd = NULL;

const Char *g_strFile = "data\\Generals.str";
const Char *g_csfFile = "data\\%s\\Generals.csf";


// PRIVATE PROTOTYPES /////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

// WinMain ====================================================================
/** Application entry point */
//=============================================================================
Int APIENTRY WinMain( HINSTANCE hInstance, HINSTANCE hPrevInstance,
                      LPSTR lpCmdLine, Int nCmdShow )
{

	// start the log
	DEBUG_INIT(DEBUG_FLAGS_DEFAULT);
	initMemoryManager();

	// save application instance
	ApplicationHInstance = hInstance;

	// allocate a new image packer system
	TheImagePacker = new ImagePacker;
	if( TheImagePacker == NULL )
		return 0;

	// initialize the system
	if( TheImagePacker->init() == FALSE )
	{

		delete TheImagePacker;
		TheImagePacker = NULL;
		return 0;

	}  // end if
		
	// load the dialog box
	DialogBox( hInstance, (LPCTSTR)IMAGE_PACKER_DIALOG, 
						 NULL, (DLGPROC)ImagePackerProc );

	// delete the image packer
	delete TheImagePacker;
	TheImagePacker = NULL;

	// close the log
	shutdownMemoryManager();
	DEBUG_SHUTDOWN();

	// all done
	return 0;

}  // end WinMain
