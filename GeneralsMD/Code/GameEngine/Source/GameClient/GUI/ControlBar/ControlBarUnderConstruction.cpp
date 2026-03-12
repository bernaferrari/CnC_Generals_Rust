// FILE: ControlBarUnderConstruction.cpp //////////////////////////////////////////////////////////
// Author: Colin Day, March 2002
// Desc:   Methods specific to the control bar under construction context
///////////////////////////////////////////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/NameKeyGenerator.h"
#include "Common/ThingTemplate.h"

#include "GameLogic/Object.h"
#include "GameLogic/Module/UpdateModule.h"

#include "GameClient/Drawable.h"
#include "GameClient/GameText.h"
#include "GameClient/ControlBar.h"
#include "GameClient/GameWindow.h"
#include "GameClient/GameWindowManager.h"
#include "GameClient/GadgetStaticText.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void ControlBar::updateConstructionTextDisplay( Object *obj )
{
	UnicodeString text;
	static UnsignedInt descID = TheNameKeyGenerator->nameToKey( "ControlBar.wnd:UnderConstructionDesc" );
	GameWindow *descWindow = TheWindowManager->winGetWindowFromId( NULL, descID );

	// santiy
	DEBUG_ASSERTCRASH( descWindow, ("Under construction window not found\n") );

	// format the message
	text.format( TheGameText->fetch( "CONTROLBAR:UnderConstructionDesc" ), 
							 obj->getConstructionPercent() );
	GadgetStaticTextSetText( descWindow, text );

	// record this as the last percentage displayed
	m_displayedConstructPercent = obj->getConstructionPercent();

}  // end updateConstructionTextDisplay

//-------------------------------------------------------------------------------------------------
/** Populate the interface for an under construction context. */
//-------------------------------------------------------------------------------------------------
void ControlBar::populateUnderConstruction( Object *objectUnderConstruction )
{

	// sanity
	if( objectUnderConstruction == NULL )
		return;

	// get our parent window
	GameWindow *parent = m_contextParent[ CP_UNDER_CONSTRUCTION ];

	// set the cancel construction button
/// @todo srj -- remove hard-coding here, please
	const CommandButton *commandButton = findCommandButton( "Command_CancelConstruction" );
	NameKeyType id;
	id = TheNameKeyGenerator->nameToKey( "ControlBar.wnd:ButtonCancelConstruction" );
	GameWindow *win = TheWindowManager->winGetWindowFromId( parent, id );

	setControlCommand( win, commandButton );
	win->winSetStatus( WIN_STATUS_USE_OVERLAY_STATES );
	
	// set the text description of what is building
	updateConstructionTextDisplay( objectUnderConstruction );

	// set the portrait for the thing being constructed
	setPortraitByObject( objectUnderConstruction );

	// and show the rally point, if it should have one,
	ExitInterface *exit = objectUnderConstruction->getObjectExitInterface();
	if( exit )
		showRallyPoint( exit->getRallyPoint() );


}  // end populateUnderConstruction

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void ControlBar::updateContextUnderConstruction( void )
{
	Object *obj = m_currentSelectedDrawable->getObject();

	// if the object is no longer under construction switch to a new appropriate context
	if( !obj->getStatusBits().test( OBJECT_STATUS_UNDER_CONSTRUCTION ) )
	{

		evaluateContextUI();
		return;

	}  // end if

	// if the construction percent has changed since what was last shown to the user update the text
	if( m_displayedConstructPercent != obj->getConstructionPercent() )
		updateConstructionTextDisplay( obj );

}  // end updatecontextUnderConstruction
