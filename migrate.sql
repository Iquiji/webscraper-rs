PGDMP     8                    x           postgres    12.2    12.1                0    0    ENCODING    ENCODING        SET client_encoding = 'UTF8';
                      false                       0    0 
   STDSTRINGS 
   STDSTRINGS     (   SET standard_conforming_strings = 'on';
                      false                       0    0 
   SEARCHPATH 
   SEARCHPATH     8   SELECT pg_catalog.set_config('search_path', '', false);
                      false                       1262    16402    postgres    DATABASE     z   CREATE DATABASE postgres WITH TEMPLATE = template0 ENCODING = 'UTF8' LC_COLLATE = 'en_US.UTF-8' LC_CTYPE = 'en_US.UTF-8';
    DROP DATABASE postgres;
                postgres    false                       0    0    DATABASE postgres    COMMENT     N   COMMENT ON DATABASE postgres IS 'default administrative connection database';
                   postgres    false    3865                       0    0    SCHEMA public    ACL     &   GRANT ALL ON SCHEMA public TO PUBLIC;
                   postgres    false    5            �            1259    38079    base_url_links    TABLE     �   CREATE TABLE public.base_url_links (
    base_url text NOT NULL,
    target_url text NOT NULL,
    weight double precision DEFAULT 0.2
);
 "   DROP TABLE public.base_url_links;
       public         heap    postgres    false            �            1259    36941    content    TABLE     �   CREATE TABLE public.content (
    type text NOT NULL,
    url text NOT NULL,
    creator_id text,
    creator_username text,
    title text,
    views bigint,
    preview_url text
);
    DROP TABLE public.content;
       public         heap    postgres    false            �            1259    19882    crawl_queue    TABLE     i   CREATE TABLE public.crawl_queue (
    url text NOT NULL,
    json jsonb,
    priority bigint NOT NULL
);
    DROP TABLE public.crawl_queue;
       public         heap    postgres    false            �            1259    38063    crawl_queue_v2    TABLE     �   CREATE TABLE public.crawl_queue_v2 (
    url text NOT NULL,
    "timestamp" timestamp without time zone,
    status text NOT NULL,
    error_count bigint DEFAULT 0 NOT NULL
);
 "   DROP TABLE public.crawl_queue_v2;
       public         heap    postgres    false            �            1259    80954    images    TABLE     a   CREATE TABLE public.images (
    url text NOT NULL,
    text text,
    text_tsvector tsvector
);
    DROP TABLE public.images;
       public         heap    postgres    false            �            1259    36935    users    TABLE     �   CREATE TABLE public.users (
    email text,
    username text NOT NULL,
    uuid text NOT NULL,
    password text,
    valid_session_id text
);
    DROP TABLE public.users;
       public         heap    postgres    false            �            1259    16408    websites    TABLE     �   CREATE TABLE public.websites (
    hostname text NOT NULL,
    added date NOT NULL,
    popularity bigint NOT NULL,
    text text NOT NULL,
    info json
);
    DROP TABLE public.websites;
       public         heap    postgres    false            �            1259    36949    websites_v2    TABLE     �   CREATE TABLE public.websites_v2 (
    url text NOT NULL,
    text text,
    last_scraped date DEFAULT make_date(1970, 1, 1),
    popularity bigint DEFAULT 0,
    json json,
    rank double precision,
    text_tsvector tsvector,
    hostname text
);
    DROP TABLE public.websites_v2;
       public         heap    postgres    false            �           2606    38100 "   base_url_links base_url_links_pkey 
   CONSTRAINT     r   ALTER TABLE ONLY public.base_url_links
    ADD CONSTRAINT base_url_links_pkey PRIMARY KEY (base_url, target_url);
 L   ALTER TABLE ONLY public.base_url_links DROP CONSTRAINT base_url_links_pkey;
       public            postgres    false    208    208            �           2606    38070 "   crawl_queue_v2 crawl_queue_v2_pkey 
   CONSTRAINT     a   ALTER TABLE ONLY public.crawl_queue_v2
    ADD CONSTRAINT crawl_queue_v2_pkey PRIMARY KEY (url);
 L   ALTER TABLE ONLY public.crawl_queue_v2 DROP CONSTRAINT crawl_queue_v2_pkey;
       public            postgres    false    207            �           2606    80961    images images_pkey 
   CONSTRAINT     Q   ALTER TABLE ONLY public.images
    ADD CONSTRAINT images_pkey PRIMARY KEY (url);
 <   ALTER TABLE ONLY public.images DROP CONSTRAINT images_pkey;
       public            postgres    false    209            �           2606    36960    websites_v2 unique_url 
   CONSTRAINT     P   ALTER TABLE ONLY public.websites_v2
    ADD CONSTRAINT unique_url UNIQUE (url);
 @   ALTER TABLE ONLY public.websites_v2 DROP CONSTRAINT unique_url;
       public            postgres    false    206            �           2606    38072    websites_v2 websites_v2_pkey 
   CONSTRAINT     [   ALTER TABLE ONLY public.websites_v2
    ADD CONSTRAINT websites_v2_pkey PRIMARY KEY (url);
 F   ALTER TABLE ONLY public.websites_v2 DROP CONSTRAINT websites_v2_pkey;
       public            postgres    false    206            �           1259    39035    crawl_queue_v2_queued    INDEX     k   CREATE INDEX crawl_queue_v2_queued ON public.crawl_queue_v2 USING btree ("timestamp" NULLS FIRST, status);
 )   DROP INDEX public.crawl_queue_v2_queued;
       public            postgres    false    207    207            �           1259    39030 	   gin_index    INDEX     H   CREATE INDEX gin_index ON public.websites_v2 USING gin (text_tsvector);
    DROP INDEX public.gin_index;
       public            postgres    false    206            �           1259    39036 	   hostnames    INDEX     E   CREATE INDEX hostnames ON public.websites_v2 USING btree (hostname);
    DROP INDEX public.hostnames;
       public            postgres    false    206            �           1259    80962 	   image_gin    INDEX     C   CREATE INDEX image_gin ON public.images USING gin (text_tsvector);
    DROP INDEX public.image_gin;
       public            postgres    false    209           