#if defined(_MSC_VER)
#pragma once
#endif

#ifndef HASH_H
#define HASH_H

#include "always.h"

class HashTableIteratorClass;


/*
** HashableClass
*/
class	HashableClass {

public:
	HashableClass( void ) : NextHash( NULL ) {}
	virtual	~HashableClass( void ) {}

	virtual	const char * Get_Key( void )				= 0;

private:
	HashableClass * NextHash;
	friend	class		HashTableClass;
	friend	class		HashTableIteratorClass;
};


/*
** HashTableClass
*/
class HashTableClass {

public:
	HashTableClass( int size );
	~HashTableClass( void );

	void					Reset( void );
	void					Add( HashableClass * entry );
	bool					Remove( HashableClass * entry );

	HashableClass *	Find( const char * key );

private:
	// HashTableSize MUST be a power of two
	int					HashTableSize;
	HashableClass * *	HashTable;

	// Convert key to a table index
	int					Hash( const char * key );

	friend	class		HashTableIteratorClass;
};


/*
**
*/
class HashTableIteratorClass
{
public:
	HashTableIteratorClass( HashTableClass & table ) : Table( table ) {}
	virtual ~HashTableIteratorClass( void ) {}

	void					First( void );
	void					Next( void );
	bool					Is_Done( void )		{ return CurrentEntry == NULL; }
	HashableClass *	Get_Current( void )	{ return CurrentEntry; }

private:
	const HashTableClass	&	Table;
	int							Index;
	HashableClass *			CurrentEntry;
	HashableClass *			NextEntry;

	void					Advance_Next( void );
};


#endif	// HASH_H