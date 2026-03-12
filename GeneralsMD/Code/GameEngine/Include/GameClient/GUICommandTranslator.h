// FILE: GUICommandTranslator.h ///////////////////////////////////////////////////////////////////
// Author: Colin Day, March 2002
// Desc:   Translator for commands activated from the selection GUI, such as special unit
//				 actions, that require additional clicks in the world like selecting a target
//				 object or location
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GUICOMMANDTRANSLATOR_H_
#define __GUICOMMANDTRANSLATOR_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "GameClient/InGameUI.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class GUICommandTranslator : public GameMessageTranslator                          
{

public:

	GUICommandTranslator( void );
	~GUICommandTranslator( void );

	virtual GameMessageDisposition translateGameMessage( const GameMessage *msg );
};	

#endif  // end __GUICOMMANDTRANSLATOR_H_


