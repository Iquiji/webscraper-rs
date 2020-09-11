--
-- PostgreSQL database dump
--

-- Dumped from database version 12.2
-- Dumped by pg_dump version 12.1

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: base_url_links; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.base_url_links (
    base_url text NOT NULL,
    target_url text NOT NULL,
    weight double precision DEFAULT 0.2
);


ALTER TABLE public.base_url_links OWNER TO postgres;

--
-- Name: content; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.content (
    type text NOT NULL,
    url text NOT NULL,
    creator_id text,
    creator_username text,
    title text,
    views bigint,
    preview_url text
);


ALTER TABLE public.content OWNER TO postgres;

--
-- Name: crawl_queue; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.crawl_queue (
    url text NOT NULL,
    json jsonb,
    priority bigint NOT NULL
);


ALTER TABLE public.crawl_queue OWNER TO postgres;

--
-- Name: crawl_queue_v2; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.crawl_queue_v2 (
    url text NOT NULL,
    "timestamp" timestamp without time zone,
    status text NOT NULL,
    error_count bigint DEFAULT 0 NOT NULL
);


ALTER TABLE public.crawl_queue_v2 OWNER TO postgres;

--
-- Name: images; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.images (
    url text NOT NULL,
    text text,
    text_tsvector tsvector
);


ALTER TABLE public.images OWNER TO postgres;

--
-- Name: users; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.users (
    email text,
    username text NOT NULL,
    uuid text NOT NULL,
    password text,
    valid_session_id text
);


ALTER TABLE public.users OWNER TO postgres;

--
-- Name: websites; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.websites (
    hostname text NOT NULL,
    added date NOT NULL,
    popularity bigint NOT NULL,
    text text NOT NULL,
    info json
);


ALTER TABLE public.websites OWNER TO postgres;

--
-- Name: websites_v2; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.websites_v2 (
    url text NOT NULL,
    text text,
    last_scraped date DEFAULT make_date(1970, 1, 1),
    popularity bigint DEFAULT 0,
    json json,
    rank double precision,
    text_tsvector tsvector,
    hostname text
);


ALTER TABLE public.websites_v2 OWNER TO postgres;

--
-- Name: base_url_links base_url_links_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.base_url_links
    ADD CONSTRAINT base_url_links_pkey PRIMARY KEY (base_url, target_url);


--
-- Name: crawl_queue_v2 crawl_queue_v2_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.crawl_queue_v2
    ADD CONSTRAINT crawl_queue_v2_pkey PRIMARY KEY (url);


--
-- Name: images images_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.images
    ADD CONSTRAINT images_pkey PRIMARY KEY (url);


--
-- Name: websites_v2 unique_url; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.websites_v2
    ADD CONSTRAINT unique_url UNIQUE (url);


--
-- Name: websites_v2 websites_v2_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.websites_v2
    ADD CONSTRAINT websites_v2_pkey PRIMARY KEY (url);


--
-- Name: crawl_queue_v2_queued; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX crawl_queue_v2_queued ON public.crawl_queue_v2 USING btree ("timestamp" NULLS FIRST, status);


--
-- Name: gin_index; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX gin_index ON public.websites_v2 USING gin (text_tsvector);


--
-- Name: hostnames; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX hostnames ON public.websites_v2 USING btree (hostname);


--
-- Name: image_gin; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX image_gin ON public.images USING gin (text_tsvector);


--
-- Name: SCHEMA public; Type: ACL; Schema: -; Owner: postgres
--

GRANT ALL ON SCHEMA public TO PUBLIC;


--
-- PostgreSQL database dump complete
--

