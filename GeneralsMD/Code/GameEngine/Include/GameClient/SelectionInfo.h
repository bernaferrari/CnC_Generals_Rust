// FILE: GameClient/ContextSensitiveTranslator.h //////////////////////////////////////////////////
// Contains the SelectionInfo structure and the contextCommandForNewSelection
// Author: John McDonald, Jr, October 2002

#pragma once

#ifndef __SELECTIONINFO_H__
#define __SELECTIONINFO_H__

#include "GameClient/InGameUI.h"

// This structure gives you a rough idea about the counts of the kinds of guys in the 
// current selection, and in the selection that would be made.
struct SelectionInfo
{
	Int currentCountEnemies;
	Int currentCountCivilians;
	Int currentCountMine;
	Int currentCountMineInfantry;
	Int currentCountMineBuildings;
	Int currentCountFriends;

	Int newCountEnemies;
	Int newCountCivilians;
	Int newCountMine;
	Int newCountMineBuildings;
	Int newCountFriends;
	Int newCountGarrisonableBuildings;
	Int newCountCrates;

	Bool selectEnemies;
	Bool selectCivilians;
	Bool selectMine;
	Bool selectMineBuildings;
	Bool selectFriends;


	SelectionInfo(); 
};

//-------------------------------------------------------------------------------------------------
struct PickDrawableStruct
{
	// List to fill with Drawables. This should be provided by the caller.
	DrawableList *drawableListToFill;
	Bool forceAttackMode;
	
	// Note, this is OR'd with the things we are attempting to select.
	KindOfMaskType kindofsToMatch;

	PickDrawableStruct();
};

//-------------------------------------------------------------------------------------------------
extern Bool contextCommandForNewSelection(const DrawableList *currentlySelectedDrawables, 
																					const DrawableList *newlySelectedDrawables,
																					SelectionInfo *outSelectionInfo,
																					Bool selectionIsPoint);


//-------------------------------------------------------------------------------------------------
// Returns ORed picktypes.
extern UnsignedInt getPickTypesForContext(Bool forceAttackMode);

//-------------------------------------------------------------------------------------------------
// Returns ORed picktypes based on the current selection.
extern UnsignedInt getPickTypesForCurrentSelection(Bool forceAttackMode);


//-------------------------------------------------------------------------------------------------
// expects ORed picktypes.
extern void translatePickTypesToKindof(UnsignedInt pickTypes, KindOfMaskType& outmask);

//-------------------------------------------------------------------------------------------------
// Given a drawable, add it to an stl list. Useful for iterateDrawablesInRegion.
// userData should be a pointer to a PickDrawableStruct, which is defined in 
// above.
extern Bool addDrawableToList( Drawable *draw, void *userData );


#endif /* __SELECTIONINFO_H__ */
