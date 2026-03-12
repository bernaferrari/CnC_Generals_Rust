// FILE: SupplyWarehouseCreate.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood Feb 2002
// Desc:   When a Supply Center is created, it needs to update all the Resource brains in all players
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Player.h"
#include "Common/PlayerList.h"
#include "Common/ResourceGatheringManager.h"
#include "Common/Xfer.h"
#include "GameLogic/Module/SupplyWarehouseCreate.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SupplyWarehouseCreate::SupplyWarehouseCreate( Thing *thing, const ModuleData* moduleData ) : CreateModule( thing, moduleData )
{

}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SupplyWarehouseCreate::~SupplyWarehouseCreate( void )
{

} 

//-------------------------------------------------------------------------------------------------
void SupplyWarehouseCreate::onCreate( void )
{
	// Warehouses are never Built.
	if( ThePlayerList == NULL )
		return;

	for( Int playerIndex = ThePlayerList->getPlayerCount() - 1; playerIndex >= 0; playerIndex-- )
	{
		Player *currentPlayer = ThePlayerList->getNthPlayer( playerIndex );
		if( currentPlayer == NULL )
			continue;
		ResourceGatheringManager *manager = currentPlayer->getResourceGatheringManager();
		if( manager == NULL )
			continue;
		manager->addSupplyWarehouse( getObject() );
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void SupplyWarehouseCreate::crc( Xfer *xfer )
{

	// extend base class
	CreateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void SupplyWarehouseCreate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	CreateModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void SupplyWarehouseCreate::loadPostProcess( void )
{

	// extend base class
	CreateModule::loadPostProcess();

}  // end loadPostProcess
