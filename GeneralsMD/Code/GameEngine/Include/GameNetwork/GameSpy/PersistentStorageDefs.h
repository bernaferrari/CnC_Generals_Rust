// FILE: PersistentStorageDefs.h //////////////////////////////////////////////////////
// Generals GameSpy Persistent Storage definitions
// Author: Matthew D. Campbell, July 2002

#pragma once

#ifndef __PERSISTENTSTORAGEDEFS_H__
#define __PERSISTENTSTORAGEDEFS_H__

enum LocaleType
{
    LOC_UNKNOWN = 0,
    LOC_MIN = 1,
    LOC_MAX = 37
};

void HandlePersistentStorageResponses(void);
void UpdateLocalPlayerStats(void);

void SetLookAtPlayer( Int id, AsciiString nick );
void PopulatePlayerInfoWindows( AsciiString parentWindowName );

#endif // __PERSISTENTSTORAGEDEFS_H__
