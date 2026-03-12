///////////////////////////////////////////////////////////////////////////////////////
// FILE: WOLLoginMenu.cpp
// Author: Chris Huybregts, November 2001
// Description: Lan Lobby Menu
///////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/GameEngine.h"
#include "GameClient/WindowLayout.h"
#include "GameClient/Gadget.h"
#include "GameClient/Shell.h"
#include "GameClient/KeyDefs.h"
#include "GameClient/GameWindowManager.h"
#include "GameClient/GadgetListBox.h"
#include "GameClient/GadgetTextEntry.h"
//#include "GameNetwork/WOL.h"
//#include "GameNetwork/WOLmenus.h"



// PRIVATE DATA ///////////////////////////////////////////////////////////////////////////////////
// window ids ------------------------------------------------------------------------------
static NameKeyType parentWOLStatusID = NAMEKEY_INVALID;
static NameKeyType buttonDisconnectID = NAMEKEY_INVALID;

// Window Pointers ------------------------------------------------------------------------
static GameWindow *parentWOLStatus = NULL;
static GameWindow *buttonDisconnect = NULL;
GameWindow *progressTextWindow = NULL;

//-------------------------------------------------------------------------------------------------
/** Initialize the WOL Status Menu */
//-------------------------------------------------------------------------------------------------
void WOLStatusMenuInit( WindowLayout *layout, void *userData )
{
	parentWOLStatusID = TheNameKeyGenerator->nameToKey( AsciiString( "WOLStatusMenu.wnd:WOLStatusMenuParent" ) );
	buttonDisconnectID = TheNameKeyGenerator->nameToKey( AsciiString( "WOLStatusMenu.wnd:ButtonDisconnect" ) );
	parentWOLStatus = TheWindowManager->winGetWindowFromId( NULL, parentWOLStatusID );
	buttonDisconnect = TheWindowManager->winGetWindowFromId( NULL,  buttonDisconnectID);

	progressTextWindow = TheWindowManager->winGetWindowFromId( NULL,
		TheNameKeyGenerator->nameToKey( AsciiString( "WOLStatusMenu.wnd:ListboxStatus" ) ) );
	
	// Show Menu
	layout->hide( FALSE );

	// Set Keyboard to Main Parent
	TheWindowManager->winSetFocus( parentWOLStatus );

	//progressLayout = TheShell->top();

	//WOL::raiseWOLMessageBox();
} // WOLStatusMenuInit

//-------------------------------------------------------------------------------------------------
/** WOL Status Menu shutdown method */
//-------------------------------------------------------------------------------------------------
void WOLStatusMenuShutdown( WindowLayout *layout, void *userData )
{

	// hide menu
	layout->hide( TRUE );

	// our shutdown is complete
	TheShell->shutdownComplete( layout );

	//progressLayout = NULL;

	//WOL::raiseWOLMessageBox();
}  // WOLStatusMenuShutdown


//-------------------------------------------------------------------------------------------------
/** WOL Status Menu update method */
//-------------------------------------------------------------------------------------------------
void WOLStatusMenuUpdate( WindowLayout * layout, void *userData)
{
	//if (WOL::TheWOL)
		//WOL::TheWOL->update();
}// WOLStatusMenuUpdate

//-------------------------------------------------------------------------------------------------
/** WOL Status Menu input callback */
//-------------------------------------------------------------------------------------------------
WindowMsgHandledType WOLStatusMenuInput( GameWindow *window, UnsignedInt msg,
																			 WindowMsgData mData1, WindowMsgData mData2 )
{
	switch( msg ) 
	{

		// --------------------------------------------------------------------------------------------
		case GWM_CHAR:
		{
			UnsignedByte key = mData1;
			UnsignedByte state = mData2;

			switch( key )
			{

				// ----------------------------------------------------------------------------------------
				case KEY_ESC:
				{
					
					//
					// send a simulated selected event to the parent window of the
					// back/exit button
					//
					if( BitTest( state, KEY_STATE_UP ) )
					{
						TheWindowManager->winSendSystemMsg( window, GBM_SELECTED, 
																							(WindowMsgData)buttonDisconnect, buttonDisconnectID );

					}  // end if

					// don't let key fall through anywhere else
					return MSG_HANDLED;

				}  // end escape

			}  // end switch( key )

		}  // end char

	}  // end switch( msg )

	return MSG_IGNORED;
}// WOLStatusMenuInput

//-------------------------------------------------------------------------------------------------
/** WOL Status Menu window system callback */
//-------------------------------------------------------------------------------------------------
WindowMsgHandledType WOLStatusMenuSystem( GameWindow *window, UnsignedInt msg, 
														 WindowMsgData mData1, WindowMsgData mData2 )
{
	UnicodeString txtInput;

	switch( msg )
	{
		
		
		case GWM_CREATE:
			{
				
				break;
			} // case GWM_DESTROY:

		case GWM_DESTROY:
			{
				break;
			} // case GWM_DESTROY:

		case GWM_INPUT_FOCUS:
			{	
				// if we're givin the opportunity to take the keyboard focus we must say we want it
				if( mData1 == TRUE )
					*(Bool *)mData2 = TRUE;

				return MSG_HANDLED;
			}//case GWM_INPUT_FOCUS:

		case GBM_SELECTED:
			{
				/*
				GameWindow *control = (GameWindow *)mData1;
				Int controlID = control->winGetWindowId();

				if ( controlID == buttonDisconnectID )
				{
					//TheShell->pop();
					if (WOL::TheWOL->setState( WOL::WOLAPI_FATAL_ERROR ))
					{
						WOL::TheWOL->addCommand( WOL::WOLCOMMAND_RESET );  // don't display an error, log out, or anything
					}

				} //if ( controlID == buttonDisconnect )
				*/
				break;
			}// case GBM_SELECTED:
	
		case GEM_EDIT_DONE:
			{
				break;
			}
		default:
			return MSG_IGNORED;

	}//Switch

	return MSG_HANDLED;
}// WOLStatusMenuSystem
