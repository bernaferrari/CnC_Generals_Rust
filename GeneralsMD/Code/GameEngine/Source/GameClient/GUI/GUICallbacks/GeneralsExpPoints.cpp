// FILE: GeneralsExpPoints.cpp /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	created:	Oct 2002
//
//	Filename: 	GeneralsExpPoints.cpp
//
//	author:		Chris Huybregts
//	
//	purpose:	File used to populate/update/show/hide the generals exp screen
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

//-----------------------------------------------------------------------------
// SYSTEM INCLUDES ////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "GameClient/ControlBar.h"
#include "GameClient/GUICallbacks.h"
#include "GameClient/GameWindow.h"
#include "GameClient/Gadget.h"
#include "GameClient/KeyDefs.h"
#include "GameClient/ControlBar.h"
#include "GameClient/InGameUI.h"
//-----------------------------------------------------------------------------
// DEFINES ////////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
WindowMsgHandledType GeneralsExpPointsInput( GameWindow *window, UnsignedInt msg,
																			WindowMsgData mData1, WindowMsgData mData2 )
{

	switch( msg ) 
	{

		// --------------------------------------------------------------------------------------------
		case GWM_MOUSE_ENTERING:
			//Get rid of any building placement mode!
			if( TheInGameUI )
			{
				TheInGameUI->placeBuildAvailable( NULL, NULL );
			}
			break;
	
		case GWM_CHAR:
		{
			UnsignedByte key = mData1;
//			UnsignedByte state = mData2;

			switch( key )
			{

				// ----------------------------------------------------------------------------------------
				case KEY_ESC:
				{
					TheControlBar->hidePurchaseScience();
					return MSG_HANDLED;
					//return MSG_IGNORED;
				}  // end escape

			}  // end switch( key )

			return MSG_HANDLED;

		}  // end char

	}

	return MSG_HANDLED;

}  // end DiplomacyInput



//-----------------------------------------------------------------------------
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
WindowMsgHandledType GeneralsExpPointsSystem( GameWindow *window, UnsignedInt msg, 
																			 WindowMsgData mData1, WindowMsgData mData2 )
{

	switch( msg ) 
	{
		//---------------------------------------------------------------------------------------------
		case GGM_FOCUS_CHANGE:
		{
			//			Bool focus = (Bool) mData1;
			//if (focus)
				//TheWindowManager->winSetGrabWindow( chatTextEntry );
			break;
		} // end focus change

		//---------------------------------------------------------------------------------------------
		case GWM_INPUT_FOCUS:
		{	
			// if we're given the opportunity to take the keyboard focus we must say we don't want it
			if( mData1 == TRUE )
				*(Bool *)mData2 = FALSE;

			return MSG_HANDLED;
		}//case GWM_INPUT_FOCUS:

		//---------------------------------------------------------------------------------------------
		case GBM_SELECTED:
		{
			GameWindow *control = (GameWindow *)mData1;
			NameKeyType controlID = (NameKeyType)control->winGetWindowId();
			static NameKeyType buttonExitID = NAMEKEY( "GeneralsExpPoints.wnd:ButtonExit" );
			if (controlID == buttonExitID)
			{
				TheControlBar->hidePurchaseScience();
			}
			else
				TheControlBar->processContextSensitiveButtonClick( control, (GadgetGameMessage)msg );
			break;

		}  // end button selected

		//---------------------------------------------------------------------------------------------
		default:
			return MSG_IGNORED;

	}  // end switch( msg )

	return MSG_HANDLED;

}  // end GeneralsExpPointsSystem

//-----------------------------------------------------------------------------
// PRIVATE FUNCTIONS //////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------

