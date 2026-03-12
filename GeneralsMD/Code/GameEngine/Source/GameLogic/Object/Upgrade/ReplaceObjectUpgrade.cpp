// FILE: ReplaceObjectUpgrade.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, July 2003
// Desc:	 UpgradeModule that creates a new Object in our exact location and then deletes our object
///////////////////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameLogic/Module/ReplaceObjectUpgrade.h"

#include "Common/Player.h"
#include "Common/ThingFactory.h"
#include "Common/ThingTemplate.h"
#include "GameLogic/AI.h"
#include "GameLogic/AIPathfind.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Module/CreateModule.h"
#include "GameLogic/Object.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void ReplaceObjectUpgradeModuleData::buildFieldParse(MultiIniFieldParse& p) 
{
  UpgradeModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "ReplaceObject",	INI::parseAsciiString,	NULL, offsetof( ReplaceObjectUpgradeModuleData, m_replaceObjectName ) },
		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ReplaceObjectUpgrade::ReplaceObjectUpgrade( Thing *thing, const ModuleData* moduleData ) : UpgradeModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ReplaceObjectUpgrade::~ReplaceObjectUpgrade( void )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void ReplaceObjectUpgrade::upgradeImplementation( )
{
	const ReplaceObjectUpgradeModuleData *data = getReplaceObjectUpgradeModuleData();

	Object *me = getObject();	

	Matrix3D myMatrix = *me->getTransformMatrix();
	Team *myTeam = me->getTeam();// Team implies player.  It is a subset.

	const ThingTemplate *replacementTemplate = TheThingFactory->findTemplate(data->m_replaceObjectName);
	if( replacementTemplate == NULL )
	{
		DEBUG_ASSERTCRASH(replacementTemplate != NULL, ("No such object '%s' in ReplaceObjectUpgrade.", data->m_replaceObjectName.str() ) );
		return;
	}

	// Remove us first since occupation of cells is apparently not a refcount, but a flag.  If I don't remove, then the new
	// thing will be placed, and then on deletion I will remove "his" marks.
	TheAI->pathfinder()->removeObjectFromPathfindMap( me );
	TheGameLogic->destroyObject(me);

	Object *replacementObject = TheThingFactory->newObject(replacementTemplate, myTeam);
	replacementObject->setTransformMatrix(&myMatrix);
	TheAI->pathfinder()->addObjectToPathfindMap( replacementObject );

	// Now onCreates were called at the constructor.  This magically created
	// thing needs to be considered as Built for Game specific stuff.
	for (BehaviorModule** m = replacementObject->getBehaviorModules(); *m; ++m)
	{
		CreateModuleInterface* create = (*m)->getCreate();
		if (!create)
			continue;
		create->onBuildComplete();
	}

	if( replacementObject->getControllingPlayer() )
	{
		replacementObject->getControllingPlayer()->onStructureConstructionComplete(me, replacementObject, FALSE);
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void ReplaceObjectUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void ReplaceObjectUpgrade::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpgradeModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void ReplaceObjectUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess
