//
// Bin.h
//


#ifndef __BIN_H
#define __BIN_H

#include "list.h"
#include "OLEString.h"

class BinItem: public ListNode
{
	int				hash;
	OLECHAR		*text1;
	int				text1size;
	OLECHAR		*text2;
	int				text2size;

	public:
	BinItem ( void *data, int hash, OLECHAR *text1, OLECHAR *text2 );
	int Same ( int chash, OLECHAR *ctext1, int size1, OLECHAR *ctext2, int size2 );

};

class Bin
{
	List			*bucket;
	int				num_buckets;
	BinItem		*sh_item;
	int				sh_size1,sh_size2;
	int				sh_hash;
	OLECHAR		*sh_text1, *sh_text2;

	int calc_hash ( OLECHAR *text );

	public:

	Bin ( int size = 256 );
	~Bin ();

	void				Clear				( void );
	void*				Get					( OLECHAR *text1, OLECHAR *text2 = NULL );
	void*				GetNext			( void );
	void				Add					( void *item, OLECHAR *text1, OLECHAR *text2 = NULL );
	BinItem*		GetBinItem	( OLECHAR *text1, OLECHAR *text2 = NULL );
	BinItem*		GetBinItem	( void *item );
	BinItem*		GetNextBinItem	( void );
	void				Remove			( void *item );
	void				Remove			( OLECHAR *text1, OLECHAR *text2 = NULL );
	void				Remove			( BinItem *item );


};


class BinIDItem: public ListNode
{
	int				id;

	public:
	BinIDItem ( void *data, int id );
	int Same ( int id );

};

class BinID
{
	List			*bucket;
	int				num_buckets;

	public:

	BinID ( int size = 256 );
	~BinID ();

	void				Clear				( void );
	void*				Get					( int id );
	void				Add					( void *item, int id  );
	BinIDItem*	GetBinIDItem	( int id );
	BinIDItem*	GetBinIDItem	( void *item );
	void				Remove			( void *item );
	void				Remove			( int id );
	void				Remove			( BinIDItem *item );


};


#endif // __BIN_H