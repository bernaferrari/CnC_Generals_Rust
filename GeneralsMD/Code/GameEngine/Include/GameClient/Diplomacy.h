// FILE: Diplomacy.h /////////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, Sept 2002
//////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DIPLOMACY_H__
#define __DIPLOMACY_H__

void PopulateInGameDiplomacyPopup( void );
void UpdateDiplomacyBriefingText(AsciiString newText, Bool clear);

typedef std::list<AsciiString> BriefingList;
BriefingList* GetBriefingTextList(void);

#endif // #ifndef __DIPLOMACY_H__
