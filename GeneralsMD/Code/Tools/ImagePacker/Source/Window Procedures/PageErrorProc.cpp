// FILE: PageErrorProc.cpp ////////////////////////////////////////////////////
//
// Project:    ImagePacker
//
// File name:  PageErrorProc.cpp
//
// Created:    Colin Day, August 2001
//
// Desc:       Window procedure for the error dialog for texture pages
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////
#include <windows.h>
#include <stdio.h>

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "ImagePacker.h"
#include "Resource.h"

// DEFINES ////////////////////////////////////////////////////////////////////

// PRIVATE TYPES //////////////////////////////////////////////////////////////

// PRIVATE DATA ///////////////////////////////////////////////////////////////

// PUBLIC DATA ////////////////////////////////////////////////////////////////

// PRIVATE PROTOTYPES /////////////////////////////////////////////////////////

// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

// PageErrorProc ==============================================================
/** Dialog proc for the error window */
//=============================================================================
BOOL CALLBACK PageErrorProc( HWND hWndDialog, UINT message,
														 WPARAM wParam, LPARAM lParam )
{

	switch( message )
	{

		// ------------------------------------------------------------------------
		case WM_INITDIALOG:
		{

			//
			// load the listbox with pages that could not be processed
			// and the reasons for it
			//
			
			// sanity
			if( TheImagePacker == NULL )
				return TRUE;

			// go through all pages
			TexturePage *page;
			HWND list = GetDlgItem( hWndDialog, LIST_PAGES );
			char buffer[ _MAX_PATH + 256 ];
			char reason[ 32 ];

			for( page = TheImagePacker->getFirstTexturePage();
					 page;
					 page = page->m_next )
			{

				// if image can't be processed find out why
				if( BitTest( page->m_status, TexturePage::PAGE_ERROR ) )
				{
					
					if( BitTest( page->m_status, TexturePage::CANT_ALLOCATE_PACKED_IMAGE ) )
						sprintf( reason, "Can't allocate image memory" );
					else if( BitTest( page->m_status, TexturePage::CANT_ADD_IMAGE_DATA ) )
						sprintf( reason, "Can't add image(s) data" );
					else if( BitTest( page->m_status, TexturePage::NO_TEXTURE_DATA ) )
						sprintf( reason, "No texture data to write" );
					else if( BitTest( page->m_status, TexturePage::ERROR_DURING_SAVE ) )
						sprintf( reason, "Error writing texture file" );
					else
						sprintf( reason, "Unknown Reason" );

					sprintf( buffer, "%s: (%dx%d) %s%d",
									 reason, page->getWidth(), page->getHeight(),
									 TheImagePacker->getOutputFile(), page->getID() );

					SendMessage( list, LB_INSERTSTRING, -1, (LPARAM)buffer );

				}  // end if

			}  // end for i

			// set the extents for the horizontal scroll bar in the listbox
			SendMessage( list, LB_SETHORIZONTALEXTENT, 1280, 0 );

			return TRUE;

		}  // end init

		// ------------------------------------------------------------------------
		case WM_COMMAND:
		{
			Int controlID = LOWORD( wParam );
//			Int notifyCode = HIWORD( wParam );
//			HWND hWndControl = (HWND)lParam;

			switch( controlID )
			{

				// --------------------------------------------------------------------
				case IDOK:
				{

					EndDialog( hWndDialog, TRUE );
					break;

				}  // end proceed

			}  // end switch

			break;

		}  // end command

	}  // end switch message

	return 0;

}  // end PageErrorProc


