///////////////////////////////////////////////////////////////////////////////////////
// FILE: WOLMessageWindow.cpp
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
#include "GameNetwork/IPEnumeration.h"
//#include "GameNetwork/WOL.h"



// PRIVATE DATA ///////////////////////////////////////////////////////////////////////////////////
// window ids ------------------------------------------------------------------------------
static NameKeyType parentWOLMessageWindowID = NAMEKEY_INVALID;
static NameKeyType buttonCancelID = NAMEKEY_INVALID;

// Window Pointers ------------------------------------------------------------------------
static GameWindow *parentWOLMessageWindow = NULL;
static GameWindow *buttonCancel = NULL;


//-------------------------------------------------------------------------------------------------
/** Initialize the WOLMessage Window */
//-------------------------------------------------------------------------------------------------
void WOLMessageWindowInit( WindowLayout *layout, void *userData )
{
	parentWOLMessageWindowID = TheNameKeyGenerator->nameToKey( AsciiString( "WOLMessageWindow.wnd:WOLMessageWindowParent" ) );
	buttonCancelID = TheNameKeyGenerator->nameToKey( AsciiString( "WOLMessageWindow.wnd:ButtonCancel" ) );
	parentWOLMessageWindow = TheWindowManager->winGetWindowFromId( NULL, parentWOLMessageWindowID );
	buttonCancel = TheWindowManager->winGetWindowFromId( NULL,  buttonCancelID);
	

	// Show Menu
	layout->hide( FALSE );

	// Set Keyboard to Main Parent
	TheWindowManager->winSetFocus( parentWOLMessageWindow );

} // WOLMessageWindowInit

//-------------------------------------------------------------------------------------------------
/** WOLMessage Window shutdown method */
//-------------------------------------------------------------------------------------------------
void WOLMessageWindowShutdown( WindowLayout *layout, void *userData )
{

	// hide menu
	layout->hide( TRUE );

	// our shutdown is complete
	TheShell->shutdownComplete( layout );
}  // WOLMessageWindowShutdown


//-------------------------------------------------------------------------------------------------
/** WOLMessage Window update method */
//-------------------------------------------------------------------------------------------------
void WOLMessageWindowUpdate( WindowLayout * layout, void *userData)
{
	/*
	if (WOL::TheWOL)
		WOL::TheWOL->update();
	*/

}// WOLMessageWindowUpdate

//-------------------------------------------------------------------------------------------------
/** WOLMessage Window input callback */
//-------------------------------------------------------------------------------------------------
WindowMsgHandledType WOLMessageWindowInput( GameWindow *window, UnsignedInt msg,
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
																							(WindowMsgData)buttonCancel, buttonCancelID );

					}  // end if

					// don't let key fall through anywhere else
					return MSG_HANDLED;

				}  // end escape

			}  // end switch( key )

		}  // end char

	}  // end switch( msg )

	return MSG_IGNORED;
}// WOLMessageWindowInput

//-------------------------------------------------------------------------------------------------
/** WOLMessage Window window system callback */
//-------------------------------------------------------------------------------------------------
WindowMsgHandledType WOLMessageWindowSystem( GameWindow *window, UnsignedInt msg, 
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
}// WOLMessageWindowSystem
