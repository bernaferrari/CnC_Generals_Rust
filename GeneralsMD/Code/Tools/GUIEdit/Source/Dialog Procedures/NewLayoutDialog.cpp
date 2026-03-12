// FILE: NewLayoutDialog.cpp //////////////////////////////////////////////////
//
// Project:    GUIEdit
//
// File name:  NewLayoutDialog.cpp
//
// Created:    Colin Day, July 2001
//
// Desc:       New layout dialog procedure
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
#include <windows.h>

// USER INCLUDES //////////////////////////////////////////////////////////////
#include "Lib/BaseType.h"
#include "Resource.h"
#include "EditWindow.h"
#include "GUIEdit.h"

// DEFINES ////////////////////////////////////////////////////////////////////

// PRIVATE TYPES //////////////////////////////////////////////////////////////

// PRIVATE DATA ///////////////////////////////////////////////////////////////

// PUBLIC DATA ////////////////////////////////////////////////////////////////

// PRIVATE PROTOTYPES /////////////////////////////////////////////////////////

// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////

// initNewLayoutDialog ========================================================
/** The new layout dialog is being shown, initialize anything we need to */
//=============================================================================
static void initNewLayoutDialog( HWND hWndDialog )
{

	// set default keyboard focus
	SetFocus( GetDlgItem( hWndDialog, IDOK ) );

}  // end initNewLayoutDialog

// NewLayoutDialogProc ========================================================
/** Dialog procedure for the new layout dialog when starting an entire
	* new layout in the editor */
//=============================================================================
LRESULT CALLBACK NewLayoutDialogProc( HWND hWndDialog, UINT message, 
																			WPARAM wParam, LPARAM lParam )
{

	switch( message )
	{

		// ------------------------------------------------------------------------
		case WM_INITDIALOG:
		{

			// initialize the values for the the dialog
			initNewLayoutDialog( hWndDialog );
			return FALSE;

		}  // end init dialog

		// ------------------------------------------------------------------------
    case WM_COMMAND:
    {

      switch( LOWORD( wParam ) )
      {

				// --------------------------------------------------------------------
        case IDOK:
				{

					// reset the editor
					TheEditor->newLayout();

					// end this dialog
					EndDialog( hWndDialog, TRUE );

          break;

				}  // end ok

				// --------------------------------------------------------------------
        case IDCANCEL:
				{

					EndDialog( hWndDialog, FALSE );
          break;

				}  // end cancel

      }  // end switch( LOWORD( wParam ) )

      return 0;

    } // end of WM_COMMAND

		// ------------------------------------------------------------------------
    case WM_CLOSE:
		{

			EndDialog( hWndDialog, FALSE );
      return 0;

		}  // end close

		// ------------------------------------------------------------------------
		default:
			return 0;

  }  // end of switch

}  // end NewLayoutDialogProc

