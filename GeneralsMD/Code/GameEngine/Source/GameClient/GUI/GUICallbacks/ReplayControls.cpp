// FILE: ReplayControls.cpp ///////////////////////////////////////////////////////////////////////
// Author: Bryan Cleveland - December 2001
// Desc: GUI Control box for the playback controls
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameClient/GameWindow.h"
#include "GameClient/Gadget.h"
#include "GameClient/GameClient.h"

//-------------------------------------------------------------------------------------------------
/** Input procedure for the control bar */
//-------------------------------------------------------------------------------------------------
WindowMsgHandledType ReplayControlInput( GameWindow *window, UnsignedInt msg,
																			WindowMsgData mData1, WindowMsgData mData2 )
{

	return MSG_IGNORED;

}  // end MapSelectMenuInput

//-------------------------------------------------------------------------------------------------
/** System callback for the control bar parent */
//-------------------------------------------------------------------------------------------------
WindowMsgHandledType ReplayControlSystem( GameWindow *window, UnsignedInt msg, 
																			 WindowMsgData mData1, WindowMsgData mData2 )
{
	switch( msg ) 
	{

		//---------------------------------------------------------------------------------------------
		case GBM_SELECTED:
		{

			break;

		}  // end button selected

		//---------------------------------------------------------------------------------------------
		default:
			return MSG_IGNORED;

	}  // end switch( msg )

	return MSG_HANDLED;

}  // end ControlBarSystem

