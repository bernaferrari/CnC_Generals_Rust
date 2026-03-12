// FILE: PopupCommunicator.cpp /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
// Project:   RTS3
//
// File name: PopupCommunicator.cpp
//
// Created:   Chris Brue, July 2002
//
// Desc:      Electronic Arts instant messaging system
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameClient/GUICallbacks.h"
#include "GameClient/GameWindowManager.h"

// PRIVATE DATA ///////////////////////////////////////////////////////////////////////////////////
static NameKeyType buttonOkID = NAMEKEY_INVALID;
static GameWindow *buttonOk = NULL;
static GameWindow *parent = NULL;

// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
/** Initialize the Popup Communicator */
//-------------------------------------------------------------------------------------------------
void PopupCommunicatorInit( WindowLayout *layout, void *userData )
{

	//set keyboard focus to main parent and set modal
	NameKeyType parentID = TheNameKeyGenerator->nameToKey("PopupCommunicator.wnd:PopupCommunicator");
	parent = TheWindowManager->winGetWindowFromId( NULL, parentID );
	TheWindowManager->winSetFocus( parent );
	TheWindowManager->winSetModal( parent );

	// get ids for our children controls
	buttonOkID = TheNameKeyGenerator->nameToKey( AsciiString("PopupCommunicator.wnd:ButtonOk") );
	buttonOk = TheWindowManager->winGetWindowFromId( parent, buttonOkID );

}  // end PopupCommunicatorInit

//-------------------------------------------------------------------------------------------------
/** Popup Communicator shutdown method */
//-------------------------------------------------------------------------------------------------
void PopupCommunicatorShutdown( WindowLayout *layout, void *userData )
{

}  // end PopupCommunicatorShutdown

//-------------------------------------------------------------------------------------------------
/** Popup Communicator update method */
//-------------------------------------------------------------------------------------------------
void PopupcommunicatorUpdate( WindowLayout *layout, void *userData )
{

}  // end PopupCommunicatorUpdate 

//-------------------------------------------------------------------------------------------------
/** Popup Communicator input callback */
//-------------------------------------------------------------------------------------------------
WindowMsgHandledType PopupCommunicatorInput( GameWindow *window, UnsignedInt msg,
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
																								(WindowMsgData)buttonOk, buttonOkID );

					}  // end if

					// don't let key fall through anywhere else
					return MSG_HANDLED;

				}  // end escape

			}  // end switch( key )

		}  // end char

	}  // end switch( msg )

	return MSG_IGNORED;

}  // end PopupCommunicatorInput

//-------------------------------------------------------------------------------------------------
/** Popup Communicator window system callback */
//-------------------------------------------------------------------------------------------------
WindowMsgHandledType PopupCommunicatorSystem( GameWindow *window, UnsignedInt msg, 
																		 WindowMsgData mData1, WindowMsgData mData2 )
{

  switch( msg ) 
	{

		// --------------------------------------------------------------------------------------------
		case GWM_CREATE:
		{

			break;

		}  // end create
    //---------------------------------------------------------------------------------------------
		case GWM_DESTROY:
		{

			break;

		}  // end case

    //----------------------------------------------------------------------------------------------
    case GWM_INPUT_FOCUS:
		{

			// if we're givin the opportunity to take the keyboard focus we must say we want it
			if( mData1 == TRUE )
				*(Bool *)mData2 = TRUE;

			break;

		}  // end input
    //---------------------------------------------------------------------------------------------
		case GBM_SELECTED:
		{
			GameWindow *control = (GameWindow *)mData1;
			Int controlID = control->winGetWindowId();
      
			if( controlID == buttonOkID )
			{
        WindowLayout *popupCommunicatorLayout = window->winGetLayout();
        popupCommunicatorLayout->destroyWindows();
				popupCommunicatorLayout->deleteInstance();
				popupCommunicatorLayout = NULL;
			}  // end if
	
			break;

		}  // end selected

		default:
			return MSG_IGNORED;

	}  // end switch

	return MSG_HANDLED;

}